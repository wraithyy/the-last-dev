//! Where the interactive quests live in the campaign zones: the turn-in reviewer at each zone's
//! far end (`npc_end`, so a finished ticket is handed in where the work ends; 15 m from the
//! boss, outside its 12 m aggro, so a turn-in never pulls it), escort starts,
//! the CTO by the penthouse elevator, and the quest props (`combat_core::world` kinds Pickup,
//! Inspect, Device). Devices and inspect targets are built here as non-solid dressing, so nav
//! and the exported geometry do not change; pickups are drawn by the office module, which
//! hides them from players whose quest does not want them. Spots were picked on the walkable
//! grid: reachable, two metres clear, five metres from other anchors (see the reachability test).

use super::kit::*;
use super::layout::yaw_towards;
use super::themes::v;

#[derive(Clone, Copy)]
enum Spot {
    Pickup,
    Inspect,
    Device,
}

/// (anchor, kind, stand x, stand z) per zone.
type Props = &'static [(&'static str, Spot, f32, f32)];

/// Extra NPC anchors of a zone: (anchor, x, z).
type Npcs = &'static [(&'static str, f32, f32)];

/// (zone, turn-in reviewer, extra NPC anchors, quest props).
const ZONES: [(&str, (f32, f32), Npcs, Props); 9] = [
    (
        "tower/1",
        (37.5, 6.0),
        &[],
        &[
            ("prop:t1_password_1", Spot::Pickup, -22.0, 19.0),
            ("prop:t1_password_2", Spot::Pickup, -25.0, -18.0),
            ("prop:t1_password_3", Spot::Pickup, 10.0, 25.0),
            ("prop:t1_password_4", Spot::Pickup, 27.0, -22.0),
            ("prop:t1_switch_a", Spot::Device, -9.0, 0.0),
            ("prop:t1_switch_b", Spot::Device, 17.0, -5.0),
            ("prop:t1_switch_c", Spot::Device, 34.0, 13.0),
        ],
    ),
    (
        "tower/2",
        (29.0, -4.0),
        &[("npc_escort", -38.0, 2.0)],
        &[
            ("prop:t2_room_panel_1", Spot::Device, -23.0, 18.0),
            ("prop:t2_room_panel_2", Spot::Device, 4.0, -10.0),
            ("prop:t2_room_panel_3", Spot::Device, 15.0, 30.0),
            ("prop:t2_firecode_1", Spot::Inspect, -25.0, -5.0),
            ("prop:t2_firecode_2", Spot::Inspect, -9.0, 20.0),
            ("prop:t2_firecode_3", Spot::Inspect, 0.0, -32.0),
            ("prop:t2_firecode_4", Spot::Inspect, 34.0, -10.0),
            ("prop:t2_commit_note_1", Spot::Inspect, -9.0, -9.0),
            ("prop:t2_commit_note_2", Spot::Inspect, 9.0, 14.0),
            ("prop:t2_commit_note_3", Spot::Inspect, 35.0, 4.0),
        ],
    ),
    (
        "tower/3",
        (25.0, 8.0),
        &[],
        &[
            ("prop:t3_screen_logo", Spot::Device, -1.0, -23.0),
            ("prop:t3_screen_font", Spot::Device, -21.0, 10.0),
            ("prop:t3_screen_ai", Spot::Device, 10.0, 20.0),
            ("prop:t3_badge_log_1", Spot::Pickup, -19.0, -13.0),
            ("prop:t3_badge_log_2", Spot::Pickup, -8.0, 14.0),
            ("prop:t3_badge_log_3", Spot::Pickup, -3.0, 32.0),
            ("prop:t3_badge_log_4", Spot::Pickup, 14.0, 2.0),
        ],
    ),
    (
        "tower/4",
        (32.0, 3.0),
        &[],
        &[
            ("prop:t4_gantt_1", Spot::Pickup, -34.0, -16.0),
            ("prop:t4_gantt_2", Spot::Pickup, 2.0, 24.0),
            ("prop:t4_gantt_3", Spot::Pickup, 15.0, -33.0),
            ("prop:t4_timesheet_1", Spot::Inspect, -21.0, 10.0),
            ("prop:t4_timesheet_2", Spot::Inspect, 0.0, -21.0),
            ("prop:t4_timesheet_3", Spot::Inspect, 20.0, 15.0),
            ("prop:t4_pipeline_1", Spot::Device, -14.0, -7.0),
            ("prop:t4_pipeline_2", Spot::Device, 19.0, -5.0),
            ("prop:t4_pipeline_3", Spot::Device, 32.0, -17.0),
        ],
    ),
    (
        "tower/5",
        (31.0, -10.5),
        &[("npc_cto", -41.0, -4.0)],
        &[
            ("prop:t5_deck_1", Spot::Inspect, -25.0, 12.0),
            ("prop:t5_deck_2", Spot::Inspect, -2.0, -8.0),
            ("prop:t5_deck_3", Spot::Inspect, 13.0, 16.0),
            ("prop:t5_wall_socket", Spot::Device, -33.0, 27.0),
            ("prop:t5_power_strip", Spot::Device, -1.0, -26.0),
            ("prop:t5_ups", Spot::Device, 34.0, 9.0),
            ("prop:t5_pr_1", Spot::Pickup, -19.0, -23.0),
            ("prop:t5_pr_2", Spot::Pickup, 0.0, 21.0),
            ("prop:t5_pr_3", Spot::Pickup, 23.0, -7.0),
        ],
    ),
    (
        "datacenter/1",
        (25.5, 8.5),
        &[],
        &[
            ("prop:dc1_host_1", Spot::Device, -29.0, -11.0),
            ("prop:dc1_host_2", Spot::Device, -8.0, 21.0),
            ("prop:dc1_host_3", Spot::Device, 26.0, -9.0),
            ("prop:dc1_fan_blade_1", Spot::Pickup, -28.0, 3.0),
            ("prop:dc1_fan_blade_2", Spot::Pickup, -11.0, -11.0),
            ("prop:dc1_fan_blade_3", Spot::Pickup, 13.0, 11.0),
            ("prop:dc1_fan_blade_4", Spot::Pickup, 0.0, -35.0),
            ("prop:dc1_shadow_1", Spot::Inspect, -16.0, 11.0),
            ("prop:dc1_shadow_2", Spot::Inspect, -16.0, -29.0),
            ("prop:dc1_shadow_3", Spot::Inspect, 1.0, 34.0),
            ("prop:dc1_core_switch_1", Spot::Device, 9.0, -1.0),
            ("prop:dc1_core_switch_2", Spot::Device, 29.0, 11.0),
        ],
    ),
    (
        "datacenter/2",
        (28.0, 0.0),
        &[("npc_escort", -48.8, 0.0)],
        &[
            ("prop:dc2_tape_1", Spot::Pickup, -29.0, -13.0),
            ("prop:dc2_tape_2", Spot::Pickup, -19.0, -30.0),
            ("prop:dc2_tape_3", Spot::Pickup, 30.0, 13.0),
            ("prop:dc2_valve_1", Spot::Device, -17.0, -11.0),
            ("prop:dc2_valve_2", Spot::Device, 4.0, 14.0),
            ("prop:dc2_valve_3", Spot::Device, 29.0, -12.0),
            ("prop:dc2_dashboard_1", Spot::Inspect, -29.0, 13.0),
            ("prop:dc2_dashboard_2", Spot::Inspect, 3.0, -14.0),
            ("prop:dc2_dashboard_3", Spot::Inspect, -7.0, -35.0),
        ],
    ),
    (
        "lab/1",
        (30.0, 10.5),
        &[],
        &[
            ("prop:lab1_whiteboard_1", Spot::Device, -37.0, 13.0),
            ("prop:lab1_whiteboard_2", Spot::Device, -3.0, -20.0),
            ("prop:lab1_whiteboard_3", Spot::Device, 8.0, 20.0),
            ("prop:lab1_hype_1", Spot::Inspect, -22.0, 15.0),
            ("prop:lab1_hype_2", Spot::Inspect, 13.0, -12.0),
            ("prop:lab1_hype_3", Spot::Inspect, -5.0, -37.0),
            ("prop:lab1_usb_1", Spot::Pickup, -12.0, -5.0),
            ("prop:lab1_usb_2", Spot::Pickup, -19.0, -34.0),
            ("prop:lab1_usb_3", Spot::Pickup, -5.0, 30.0),
            ("prop:lab1_usb_4", Spot::Pickup, 30.0, -10.0),
        ],
    ),
    (
        "lab/2",
        (18.0, 0.0),
        &[],
        &[
            ("prop:lab2_injection_1", Spot::Inspect, -18.0, 11.0),
            ("prop:lab2_injection_2", Spot::Inspect, -6.0, -20.0),
            ("prop:lab2_injection_3", Spot::Inspect, 6.0, 36.0),
            ("prop:lab2_console", Spot::Device, -25.0, -2.0),
            ("prop:lab2_vault", Spot::Device, 7.0, -35.0),
            ("prop:lab2_kill_switch", Spot::Device, 20.0, 14.0),
        ],
    ),
];

/// How far in front of its stand spot a device or inspect target sits (inside the clear
/// two-metre disk every spot has).
const REACH_OFFSET: f32 = 1.1;

pub fn place(zone: &str, p: &mut Plan) {
    let Some((_, end, extra, props)) = ZONES.iter().find(|z| z.0 == zone) else {
        return;
    };
    let spawn = p.anchor_pos("player_spawn").unwrap_or_default();
    let at = v(end.0, 0.0, end.1);
    p.anchor("npc_end", at, yaw_towards(at, spawn));
    for (name, x, z) in extra.iter() {
        let at = v(*x, 0.0, *z);
        p.anchor(name, at, yaw_towards(at, spawn));
    }
    for (anchor, kind, x, z) in props.iter() {
        let stand = v(*x, 0.0, *z);
        // Everything faces back towards the arrival, so its front reads on the way in.
        let to_spawn = (spawn - stand).normalize_or_zero();
        let front = v(-to_spawn.x, 0.0, -to_spawn.z);
        match kind {
            Spot::Pickup => p.interactable(anchor, stand + front * 0.6 + v(0.0, 0.3, 0.0), stand),
            Spot::Inspect => {
                let c = stand + front * REACH_OFFSET;
                // A hazard-striped crate with a printout on top: the thing worth a photo.
                p.boxed(v(c.x, 0.35, c.z), v(0.7, 0.7, 0.7), Mat::Hazard, false);
                p.boxed(v(c.x, 0.72, c.z), v(0.42, 0.02, 0.3), Mat::Paper, false);
                p.interactable(anchor, v(c.x, 0.6, c.z), stand);
            }
            Spot::Device => {
                let c = stand + front * REACH_OFFSET;
                // A free-standing rack terminal: a lit screen on the face towards the stand
                // spot, an amber status band and a blinking green light on top.
                p.boxed(v(c.x, 0.7, c.z), v(0.6, 1.4, 0.6), Mat::Rack, false);
                p.boxed(
                    v(c.x, 1.32, c.z),
                    v(0.64, 0.1, 0.64),
                    Mat::BlinkAmber,
                    false,
                );
                p.boxed(
                    v(c.x, 1.5, c.z),
                    v(0.12, 0.12, 0.12),
                    Mat::BlinkGreenA,
                    false,
                );
                let face = c - front * 0.31;
                let size = if front.x.abs() > front.z.abs() {
                    v(0.04, 0.42, 0.46)
                } else {
                    v(0.46, 0.42, 0.04)
                };
                p.boxed(v(face.x, 0.95, face.z), size, Mat::ScreenGlow, false);
                p.interactable(anchor, v(c.x, 1.0, c.z), stand);
            }
        }
    }
}
