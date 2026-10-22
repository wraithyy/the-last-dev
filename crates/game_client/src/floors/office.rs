//! Where the interactive office lives in each zone (`combat_core::world` props and hidden
//! NPCs): anchor `prop:<key>` is the spot a player uses a prop from, `secret:<unit>` a hidden
//! NPC. Most props dress pieces the zone builders already placed (coffee machines, vending,
//! printers, tables); the few new pieces (music button, fire alarm, stand-up whiteboard,
//! a second "days without" sign) are built here. Called from `layout::assemble`.

use super::kit::*;
use super::themes::v;
use std::f32::consts::{FRAC_PI_2, PI};

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

/// A hidden NPC that sits or sleeps: the chair is built under them, turned towards `look`.
fn secret_seated(p: &mut Plan, anchor: &'static str, x: f32, z: f32, look: (f32, f32)) {
    let at = v(x, 0.0, z);
    // The model front is the sitter's facing, which is half a turn off a unit yaw.
    let face = super::layout::yaw_towards(at, v(look.0, 0.0, look.1));
    p.prop(&CHAIR_DESK, at, face - PI, false);
    p.occupy(anchor, at);
}

/// Parked e-scooters anyone can borrow; the office module draws them (they vanish when
/// borrowed), so the campus builder leaves these four spots empty.
/// Two racks instead of four drive-by scooters: one at the edge of the arrival lawn, one in
/// Services Street.
pub const CAMPUS_SCOOTERS: [(&str, f32, f32); 4] = [
    ("prop:scooter_1", -19.0, 39.0),
    ("prop:scooter_2", -19.0, 42.2),
    ("prop:scooter_3", 50.0, 41.0),
    ("prop:scooter_4", 53.2, 41.0),
];

pub fn place(zone: &str, p: &mut Plan) {
    match zone {
        "campus" => campus(p),
        "tower/1" => {
            open_space(p);
            // `ZoneDef::rares`: the automation rule nobody owns, in the quiet north-west room.
            rare(p, -20.0, -19.5, (-12.0, -23.0));
        }
        "tower/2" => meeting_rooms(p),
        "tower/3" => sales_floor(p),
        "tower/4" => pmo(p),
        "tower/5" => penthouse(p),
        "datacenter/1" => {
            spot(p, "prop:vending_hall_a", (-43.0, 1.0, -9.8), (-43.0, -8.2));
            secret_seated(p, "secret:cable_sysadmin", -16.0, 37.0, (-8.0, 31.0));
            hall_a(p);
        }
        // The dark corner of the lobby, flickering.
        "datacenter/2" => {
            secret(p, "secret:deprecated_ghost", -41.8, 7.6, (-48.0, 0.0));
            hall_b(p);
            // The backup job blocks the corridor to the cooling room, facing the hall.
            rare(p, -10.0, -22.0, (-10.0, -10.0));
        }
        "lab/1" => {
            spot(p, "prop:whiteboard_lab", (-24.0, 1.9, 23.9), (-24.0, 21.6));
            spot(p, "prop:foosball_lab", (6.0, 0.9, -6.0), (6.0, -4.2));
            spot(p, "prop:printer_3d", (-8.0, 1.2, -19.0), (-8.0, -17.0));
            spot(p, "prop:vr_demo", (-14.0, 0.95, 19.0), (-14.0, 17.0));
        }
        _ => {}
    }
}

/// Anchor `rare_1` of an optional rare (`ZoneDef::rares`), turned towards `look`.
fn rare(p: &mut Plan, x: f32, z: f32, look: (f32, f32)) {
    let at = v(x, 0.0, z);
    p.anchor(
        "rare_1",
        at,
        super::layout::yaw_towards(at, v(look.0, 0.0, look.1)),
    );
}

/// A coffee machine on a side table; customers stand on its -X side.
fn coffee_station(p: &mut Plan, anchor: &'static str, x: f32, z: f32) {
    p.prop(&SIDE_TABLE, v(x, 0.0, z), FRAC_PI_2, true);
    p.prop(
        &COFFEE_MACHINE,
        v(x, SIDE_TABLE.size[1] * FURNITURE_SCALE, z),
        0.0,
        false,
    );
    spot(p, anchor, (x, 0.9, z), (x - 1.3, z));
}

/// A whiteboard on legs facing +Z, like the meeting rooms' stand-up board; users stand 1.6 m
/// in front.
fn whiteboard(p: &mut Plan, anchor: &'static str, x: f32, z: f32, sign: Sign) {
    p.boxed(v(x, 1.5, z), v(2.0, 1.36, 0.05), Mat::Board, false);
    p.sign(v(x, 1.5, z + 0.04), 0.0, 1.9, sign);
    for dx in [-0.9, 0.9] {
        p.boxed(
            v(x + dx, 0.7, z - 0.05),
            v(0.06, 1.4, 0.06),
            Mat::Metal,
            true,
        );
    }
    spot(p, anchor, (x, 1.45, z), (x, z + 1.6));
}

/// A speaker on a post with one big button, like the open space's elevator music.
fn speaker_post(p: &mut Plan, anchor: &'static str, x: f32, z: f32) {
    p.boxed(v(x, 0.6, z), v(0.12, 1.2, 0.12), Mat::Metal, true);
    p.boxed(v(x, 1.35, z), v(0.36, 0.42, 0.22), Mat::Dark, false);
    p.boxed(
        v(x + 0.12, 1.3, z),
        v(0.04, 0.16, 0.16),
        Mat::NeonPink,
        false,
    );
    spot(p, anchor, (x, 1.3, z), (x + 1.3, z));
}

/// A "days without" sign on two posts, readable from +Z; users stand 1.6 m in front.
fn days_post(p: &mut Plan, anchor: &'static str, x: f32, z: f32) {
    for dx in [-1.2, 1.2] {
        p.boxed(v(x + dx, 1.1, z), v(0.1, 2.2, 0.1), Mat::Metal, true);
    }
    p.sign(v(x, 2.0, z + 0.06), 0.0, 2.6, Sign::DaysAi);
    spot(p, anchor, (x, 2.0, z), (x, z + 1.6));
}

/// Sales & Marketing: the deal gong by the arrival, an espresso machine and the forecast board.
fn sales_floor(p: &mut Plan) {
    // The gong itself is part of the floor's dressing (`themes::decor3`).
    spot(p, "prop:gong_sales", (9.0, 1.45, -18.0), (9.0, -16.6));
    coffee_station(p, "prop:coffee_sales", 31.8, -22.5);
    whiteboard(p, "prop:whiteboard_pipeline", 4.0, -24.3, Sign::WhiteboardB);
}

/// The C-level penthouse: a gold espresso bar and a printer in the alcoves, and the pivot sign.
fn penthouse(p: &mut Plan) {
    coffee_station(p, "prop:coffee_exec", -25.8, 26.5);
    super::props::printer(p, -6.0, 29.6, Face::NegZ);
    spot(
        p,
        "prop:printer_board_deck",
        (-6.0, 0.8, 29.6),
        (-6.0, 28.1),
    );
    days_post(p, "prop:days_sign_exec", -24.0, -12.0);
}

/// Data Center Hall A: a label printer in the lobby and a thermal-event counter in the aisle.
fn hall_a(p: &mut Plan) {
    super::props::printer(p, -41.8, 9.2, Face::NegZ);
    spot(p, "prop:printer_hall_a", (-41.8, 0.8, 9.2), (-41.8, 7.7));
    days_post(p, "prop:days_sign_hall_a", -8.0, -5.6);
}

/// Data Center Hall B: vending by the campus door, the incident bridge's hold music and the
/// incident timeline.
fn hall_b(p: &mut Plan) {
    super::props::vending(p, -52.5, -8.6, Face::PosZ);
    spot(p, "prop:vending_hall_b", (-52.5, 1.0, -8.6), (-52.5, -7.0));
    speaker_post(p, "prop:bridge_line_hall_b", -28.5, -5.5);
    whiteboard(p, "prop:whiteboard_hall_b", 4.0, -5.6, Sign::WhiteboardC);
}

fn campus(p: &mut Plan) {
    // The chrome machine on the cafeteria counter; customers stand on the south side.
    spot(p, "prop:coffee_campus", (-37.0, 1.45, 7.8), (-37.0, 9.3));
    spot(p, "prop:billboard_campus", (-8.0, 6.5, 47.2), (-8.0, 38.0));
    spot(p, "prop:days_sign_campus", (-7.0, 3.1, -8.85), (-7.0, -6.5));
    spot(p, "prop:ai_car", (-39.0, 0.9, 36.0), (-39.0, 38.8));
    spot(p, "prop:bike_rack", (14.75, 0.5, -14.0), (14.75, -12.4));
    // The bench's two slots: the corner's dev holds the east one, /sit takes the other.
    spot(p, "prop:smoking_bench", (46.0, 0.5, 25.8), (46.0, 24.4));
    p.occupy("npc_4", v(46.45, 0.5, 25.8));
    for (anchor, x, z) in CAMPUS_SCOOTERS {
        spot(p, anchor, (x, 0.5, z), (x + 1.3, z));
    }
    // Behind the reception kiosk, back to the plaza, reading.
    secret_seated(p, "secret:docs_intern", 10.0, -5.8, (10.0, -10.0));
    // Sprint mode: the planning board and its terminal in Services Street, with the other menus.
    let (x, z) = (56.0, 14.0);
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
    let (x, z) = (64.0, 14.0);
    p.boxed(v(x, 0.6, z), v(0.7, 1.2, 0.5), Mat::Dark, true);
    p.boxed(v(x, 1.45, z + 0.05), v(0.66, 0.5, 0.06), Mat::Dark, false);
    p.sign(v(x, 1.45, z + 0.09), 0.0, 0.62, Sign::SprintTerminal);
    spot(p, "prop:sprint_terminal", (x, 1.2, z), (x, z + 1.5));
    // Biscuit naps on a blanket beside the Daily Standup board, the only attendee who never
    // asks for an update.
    let (dx, dz) = (14.8, 15.4);
    office_dog(p, dx, dz, -0.6);
    spot(p, "prop:office_dog", (dx, 0.4, dz), (dx - 0.4, dz + 1.4));
    live_world(p);
}

/// Where the night shift (`live.night`, anchors `night_<n>`) waits on the campus: the edges of
/// the lawn, 60 m or more from the arrival and 20 m from every door.
pub const NIGHT_SPOTS: [(&str, f32, f32); 7] = [
    ("night_1", -45.0, -64.0),
    ("night_2", 74.0, 74.0),
    ("night_3", -66.0, -6.0),
    ("night_4", 48.0, -38.0),
    ("night_5", -40.0, 118.0),
    ("night_6", 52.0, 13.0),
    // The Hard Deadline, the night's mini-elite, alone on the far west lawn by the car park.
    ("night_7", -80.0, 36.0),
];

/// The live world on the campus: the Daily Standup board with its Scrum Master Bot on the
/// plaza's east side, the Board Meeting's spot on the east lawn, the night-shift anchors and
/// the office windows that light up after dusk.
fn live_world(p: &mut Plan) {
    let (x, z) = (9.0, 12.0);
    p.boxed(v(x, 1.6, z), v(2.6, 1.5, 0.06), Mat::Board, true);
    p.sign(v(x, 1.6, z + 0.05), 0.0, 2.5, Sign::ScrumBoard);
    for dx in [-1.2, 1.2] {
        p.boxed(
            v(x + dx, 0.8, z - 0.06),
            v(0.07, 1.6, 0.07),
            Mat::Metal,
            false,
        );
    }
    spot(p, "prop:scrum_board", (x, 1.6, z), (x, z + 2.0));
    company_calendar(p);
    let bot = v(x + 2.9, 0.0, z + 1.4);
    p.anchor(
        "scrum_bot",
        bot,
        super::layout::yaw_towards(bot, v(x, 0.0, z + 2.5)),
    );
    // A level-10 meeting does not belong in the level-1 front yard: the open field between the
    // gym and the Co-Living lane, a shuttle hop from anywhere.
    let boss = v(150.0, 0.0, 82.0);
    p.anchor(
        "world_boss",
        boss,
        super::layout::yaw_towards(boss, v(100.0, 0.0, 44.0)),
    );
    for (name, x, z) in NIGHT_SPOTS {
        let at = v(x, 0.0, z);
        p.anchor(name, at, super::layout::yaw_towards(at, v(0.0, 0.0, 0.0)));
    }
    lit_windows(p);
}

/// The Company Calendar at the head of Services Street, with the other menus: a wall calendar
/// with a red header and binder rings.
/// `door:<venue>` is where a minigame group comes back to (no door, no trigger: the minigame
/// frame moves the group).
fn company_calendar(p: &mut Plan) {
    let (x, z) = (48.0, 14.0);
    p.boxed(v(x, 1.55, z), v(2.6, 1.7, 0.06), Mat::Board, true);
    p.sign(v(x, 1.5, z + 0.05), 0.0, 2.5, Sign::Calendar);
    super::entrances::calendar_pavilion(p, x, z);
    for dx in [-1.2, 1.2] {
        p.boxed(
            v(x + dx, 0.8, z - 0.06),
            v(0.07, 1.6, 0.07),
            Mat::Metal,
            false,
        );
    }
    spot(p, "prop:company_calendar", (x, 1.6, z), (x, z + 2.0));
    for door in [
        "door:teambuilding/1",
        "door:review/1",
        "door:reorg/1",
        "door:migration/1",
        "door:hackathon/1",
        "door:openspace/1",
        "door:home/1",
    ] {
        p.anchor(door, v(x, 0.0, z + 2.2), 0.0);
    }
}

/// The lab's window ribbon (the tower lights its own windows, `tower.rs`); `Mat::Window` glows
/// after dusk.
fn lit_windows(p: &mut Plan) {
    let (c, s) = super::campus::LAB;
    for sz in [-1.0, 1.0] {
        p.boxed(
            v(c.x, 3.2, c.y + sz * (s.z / 2.0 + 0.02)),
            v(s.x - 2.4, 1.8, 0.04),
            Mat::Window,
            false,
        );
    }
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
    // The orange bean bag of the lounge trio (`themes`); the nap sits on its seat.
    spot(
        p,
        "prop:beanbag_open_space",
        (-14.0, 0.45, 5.5),
        (-13.0, 4.2),
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
    secret_seated(p, "secret:last_human_qa", -12.0, -23.0, (-6.0, -23.0));
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
    // The quiet room: a glass focus booth nobody may book, door facing the corridor.
    let (qx, qz) = (-37.0, 8.5);
    quiet_booth(p, qx, qz);
    spot(p, "prop:quiet_room", (qx, 1.1, qz), (qx, qz - 1.6));
}

/// A freestanding glass phone booth (1.6 m square), open towards -Z, with a stool inside and a
/// green "free" light that is never wrong because nobody is allowed in.
fn quiet_booth(p: &mut Plan, x: f32, z: f32) {
    const H: f32 = 2.2;
    const S: f32 = 1.6;
    p.boxed(v(x, 0.03, z), v(S, 0.06, S), Mat::Dark, false);
    p.boxed(v(x, H, z), v(S + 0.1, 0.08, S + 0.1), Mat::Wood, false);
    p.boxed(v(x, H / 2.0, z + S / 2.0), v(S, H, 0.05), Mat::Wood, true);
    for side in [-1.0, 1.0] {
        p.boxed(
            v(x + side * S / 2.0, H / 2.0, z),
            v(0.05, H, S),
            Mat::Glass,
            true,
        );
        p.boxed(
            v(x + side * (S / 2.0 - 0.04), H / 2.0, z - S / 2.0),
            v(0.08, H, 0.08),
            Mat::Metal,
            true,
        );
    }
    p.boxed(v(x, 0.25, z + 0.3), v(0.45, 0.5, 0.45), Mat::Fabric1, false);
    p.boxed(
        v(x, 1.0, z + S / 2.0 - 0.06),
        v(0.7, 0.04, 0.3),
        Mat::Wood,
        false,
    );
    p.boxed(
        v(x, H - 0.12, z - S / 2.0),
        v(0.5, 0.1, 0.04),
        Mat::LedGreen,
        false,
    );
}

/// The office dog, lying down: brown body, darker ears and a tail mid-wag.
fn office_dog(p: &mut Plan, x: f32, z: f32, yaw: f32) {
    p.framed(v(x, 0.0, z), yaw, |p| {
        p.boxed(v(0.0, 0.22, 0.0), v(0.36, 0.28, 0.72), Mat::Beige, true);
        p.boxed(v(0.0, 0.42, -0.42), v(0.3, 0.28, 0.28), Mat::Beige, false);
        p.boxed(v(0.0, 0.36, -0.6), v(0.16, 0.12, 0.14), Mat::Wood, false);
        p.boxed(v(0.0, 0.4, -0.68), v(0.06, 0.05, 0.03), Mat::Dark, false);
        for side in [-1.0, 1.0] {
            p.boxed(
                v(side * 0.15, 0.5, -0.38),
                v(0.06, 0.16, 0.12),
                Mat::Cardboard,
                false,
            );
            p.boxed(
                v(side * 0.09, 0.47, -0.565),
                v(0.04, 0.04, 0.02),
                Mat::Dark,
                false,
            );
            p.boxed(
                v(side * 0.12, 0.06, -0.38),
                v(0.1, 0.12, 0.26),
                Mat::Beige,
                false,
            );
        }
        p.boxed(v(0.08, 0.3, 0.42), v(0.06, 0.06, 0.28), Mat::Beige, false);
        p.boxed(v(0.25, 0.04, 0.1), v(0.5, 0.02, 0.9), Mat::Fabric2, false);
    });
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
    secret_seated(p, "secret:cobol_contractor", -19.5, 16.5, (-14.0, 16.5));
}

#[cfg(test)]
mod tests {
    use crate::floors::build_zone;
    use crate::floors::testgrid::Grid;

    /// A seated NPC's anchor is exactly where its seat puts a standard unit's feet, and every
    /// other NPC or hidden NPC stands clear of solid geometry (no one inside a counter).
    #[test]
    fn npc_anchors_sit_on_seats_or_stand_clear() {
        let scale = crate::seat::model_scale(crate::config::PLAYER_HEIGHT);
        let mut problems = Vec::new();
        for zone in super::super::zones::ZONE_IDS {
            let Some(l) = build_zone(zone) else { continue };
            for (name, pos, _) in &l.anchors {
                if !(name.starts_with("npc_") || name.starts_with("secret:")) {
                    continue;
                }
                let seated = l
                    .seats
                    .iter()
                    .any(|s| s.taken && s.origin(scale).distance(*pos) < 0.01);
                let inside = l.blockers.iter().any(|b| {
                    pos.x > b.min.x - 0.25
                        && pos.x < b.max.x + 0.25
                        && pos.z > b.min.z - 0.25
                        && pos.z < b.max.z + 0.25
                        && b.max.y > 0.3
                        && b.min.y < 1.5
                });
                if !seated && inside {
                    problems.push(format!("{zone}: {name} at {pos} is inside geometry"));
                }
            }
            for s in l.seats.iter().filter(|s| s.taken) {
                let anchored = l
                    .anchors
                    .iter()
                    .any(|(_, p, _)| s.origin(scale).distance(*p) < 0.01);
                if !anchored {
                    problems.push(format!("{zone}: taken seat at {} has no sitter", s.pos));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// Every sittable prop is built so its seat surface is the chibi seat height.
    #[test]
    fn seats_are_at_the_chibi_seat_height() {
        let mut n = 0;
        for zone in super::super::zones::ZONE_IDS {
            let Some(l) = build_zone(zone) else { continue };
            for s in &l.seats {
                n += 1;
                let ground = l.terrain.height(s.pos.x, s.pos.z);
                assert!(
                    (s.pos.y - ground - crate::seat::SEAT_TOP).abs() < 0.03,
                    "{zone}: seat at {} is {} m high",
                    s.pos,
                    s.pos.y
                );
            }
        }
        assert!(n > 20, "only {n} seats registered");
    }

    /// The live world's campus anchors (the Scrum Master Bot, the Board Meeting, the night
    /// shift) exist where the content wants them and can be walked to.
    #[test]
    fn live_world_anchors_are_reachable() {
        let c = match combat_core::Content::from_ron(include_str!("../../../../content/game.ron")) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        let Some(l) = build_zone("campus") else {
            panic!("no campus")
        };
        let g = Grid::from(
            &l,
            l.anchor("player_spawn")
                .map_or(bevy::prelude::Vec3::ZERO, |a| a.0),
        );
        let mut wanted: Vec<String> = c
            .live
            .night
            .spawns
            .iter()
            .map(|s| s.anchor.clone())
            .collect();
        wanted.extend(c.live.board.iter().map(|b| b.anchor.clone()));
        wanted.extend(c.world_boss().map(|b| b.anchor.clone()));
        assert!(wanted.len() >= 8, "{wanted:?}");
        let spawn = l.anchor("player_spawn").map(|a| a.0);
        for name in wanted {
            match l.anchor(&name) {
                None => panic!("campus: no anchor {name}"),
                Some((pos, _)) => {
                    assert!(g.at(pos).is_finite(), "{name} at {pos} unreachable");
                    if name.starts_with("night_") {
                        // Nobody gets ambushed at the car park.
                        let d = spawn.map_or(f32::MAX, |s| s.distance(pos));
                        assert!(d > 20.0, "{name} is {d} m from the arrival");
                    }
                }
            }
        }
    }

    /// Every prop of the content sits in its zone's layout, at a stand spot a player can reach,
    /// and every hidden NPC and optional rare has its anchor.
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
                )
                // Rares skip a missing anchor in the sim, so the layout must not miss one.
                .chain(
                    c.zone(zone)
                        .into_iter()
                        .flat_map(|z| z.rares.iter().map(|r| r.anchor.clone())),
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
