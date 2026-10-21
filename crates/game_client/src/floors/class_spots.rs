//! Where the class props live (`combat_core::world` kind `Class`): a free-standing console of
//! the same family as the quest devices, so nav and the exported geometry only gain the
//! anchors. The class-coloured marker over it is drawn by the office module. Spots were picked
//! on the walkable grid (reachable, 2.3 m clear, 6 m from other anchors) like `quest_spots`.

use super::kit::*;
use super::themes::v;

/// (zone, anchor, stand x, stand z).
const SPOTS: [(&str, &str, f32, f32); 16] = [
    ("datacenter/1", "prop:devops_rack", -22.0, -3.0),
    ("datacenter/1", "prop:cobol_mainframe", -2.0, 4.0),
    ("tower/4", "prop:devops_pipeline", -49.0, 7.0),
    ("tower/4", "prop:backend_vault", 25.0, 3.0),
    ("parking_p3", "prop:devops_kiosk", -42.0, -5.0),
    ("campus", "prop:fe_billboard", 77.0, 23.0),
    ("campus", "prop:qa_escalator", -78.0, 7.0),
    ("tower/2", "prop:fe_elevator", -33.0, -2.0),
    ("tower/2", "prop:sec_tailgate", 17.0, 4.0),
    ("tower/3", "prop:fe_posters", -14.0, -3.0),
    ("datacenter/2", "prop:be_vending", -18.0, 4.0),
    ("datacenter/2", "prop:ds_dashboard", 16.0, 1.0),
    ("lab/1", "prop:qa_glitch_wall", -34.0, -6.0),
    ("tower/1", "prop:ds_ab_coffee", -10.0, 19.0),
    ("tower/1", "prop:sec_sticky", -38.0, -10.0),
    ("lab/2", "prop:cobol_microfiche", -48.0, 1.0),
];

/// How far in front of its stand spot the console sits (inside the clear disk of the spot).
const REACH_OFFSET: f32 = 1.1;

pub fn place(zone: &str, p: &mut Plan) {
    let spawn = p.anchor_pos("player_spawn").unwrap_or_default();
    for (_, anchor, x, z) in SPOTS.iter().filter(|s| s.0 == zone) {
        let stand = v(*x, 0.0, *z);
        let to_spawn = (spawn - stand).normalize_or_zero();
        let front = v(-to_spawn.x, 0.0, -to_spawn.z);
        let c = stand + front * REACH_OFFSET;
        p.boxed(v(c.x, 0.7, c.z), v(0.6, 1.4, 0.6), Mat::Rack, false);
        p.boxed(
            v(c.x, 1.32, c.z),
            v(0.64, 0.1, 0.64),
            Mat::BlinkAmber,
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
