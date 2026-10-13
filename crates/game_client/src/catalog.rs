//! Unit key -> presentation: model, size, name, WoW-style title and material
//! paints. Models come from the Quaternius Ultimate Animated Character Pack;
//! a handful of base GLBs are reused across the cast through runtime paints
//! (shirt, skin, hair ...), so the download stays small.

#![allow(dead_code)] // contract surface for the combat bridge, wired later

use crate::config::MODEL_NATIVE_HEIGHT;
use bevy::prelude::*;

/// Recolours one named glTF material of a character (`Skin`, `Shirt`, `Hair`, ...).
#[derive(Clone, Copy, Debug)]
pub struct Paint {
    pub slot: &'static str,
    /// sRGB, 0..1.
    pub rgb: [f32; 3],
    /// Emissive as well: screen faces for the robots.
    pub glow: bool,
}

const fn paint(slot: &'static str, r: f32, g: f32, b: f32) -> Paint {
    Paint {
        slot,
        rgb: [r, g, b],
        glow: false,
    }
}

const fn glow(slot: &'static str, r: f32, g: f32, b: f32) -> Paint {
    Paint {
        slot,
        rgb: [r, g, b],
        glow: true,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UnitVisual {
    /// Asset path of the character GLB.
    pub model: &'static str,
    /// Uniform scale for the model (native height is `MODEL_NATIVE_HEIGHT`).
    pub scale: f32,
    /// Standing height in metres, for nameplates and floating numbers.
    pub height: f32,
    pub name: &'static str,
    pub title: &'static str,
    pub paints: &'static [Paint],
}

/// Idle behaviour of a unit that is not fighting; the combat bridge can map
/// `UnitState.idle_pose` onto this component.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IdlePose {
    #[default]
    Standing,
    Chatting,
    /// Seated at a desk, upright.
    Sitting,
    /// Seated, slumped, with floating Z's.
    Sleeping,
}

const SUIT_M: &str = "models/characters/Suit_Male.glb";
/// Suit_Male with short greying hair and a receding hairline (scripts/ceo_hair.py).
const SUIT_M_BALD: &str = "models/characters/Suit_Male_Bald.glb";
const SUIT_F: &str = "models/characters/Suit_Female.glb";
const CASUAL3: &str = "models/characters/Casual3_Male.glb";
/// Player: Casual2 body with baked colours and hair split into thinning stages (`hair.rs`).
const ENGINEER: &str = "models/characters/Engineer.glb";
const CASUAL_F: &str = "models/characters/Casual_Female.glb";
const CASUAL2: &str = "models/characters/Casual2_Male.glb";
const CASUAL_M: &str = "models/characters/Casual_Male.glb";
const DOC_M: &str = "models/characters/Doctor_Male_Young.glb";
const DOC_F: &str = "models/characters/Doctor_Female_Old.glb";
const WORKER: &str = "models/characters/Worker_Male.glb";

/// Every character GLB the catalog references.
pub const CHARACTER_MODELS: [&str; 11] = [
    ENGINEER,
    SUIT_M,
    SUIT_M_BALD,
    SUIT_F,
    CASUAL3,
    CASUAL_F,
    CASUAL2,
    CASUAL_M,
    DOC_M,
    DOC_F,
    WORKER,
];

const fn v(
    model: &'static str,
    height: f32,
    name: &'static str,
    title: &'static str,
    paints: &'static [Paint],
) -> UnitVisual {
    UnitVisual {
        model,
        scale: height / MODEL_NATIVE_HEIGHT,
        height,
        name,
        title,
        paints,
    }
}

static TABLE: &[(&str, UnitVisual)] = &[
    (
        "senior_engineer",
        v(
            ENGINEER,
            1.8,
            "Senior Engineer",
            "<Last of the Seniors>",
            &[],
        ),
    ),
    (
        "intern",
        v(CASUAL_F, 1.62, "AI-Native Intern", "<Prompt Engineer>", &[]),
    ),
    (
        "it_support",
        v(
            CASUAL2,
            1.75,
            "IT Support",
            "<Please Open a Ticket>",
            &[
                paint("Shirt", 0.6, 0.58, 0.52),
                paint("Pants", 0.25, 0.25, 0.28),
                paint("Skin", 0.8, 0.6, 0.45),
            ],
        ),
    ),
    (
        "meatproxy",
        v(
            CASUAL_M,
            1.8,
            "MeatProxy",
            "<Human API>",
            &[
                paint("Shirt", 1.0, 0.5, 0.1),
                paint("Pants", 0.2, 0.2, 0.25),
                paint("Hair", 0.08, 0.08, 0.08),
            ],
        ),
    ),
    (
        "vibe_junior",
        v(
            CASUAL2,
            1.7,
            "Vibe-Coding Junior",
            "<Never Read It>",
            &[
                paint("Shirt", 0.55, 0.3, 0.95),
                paint("Skin", 0.95, 0.78, 0.65),
            ],
        ),
    ),
    (
        "copilot_drone",
        v(
            CASUAL_M,
            1.45,
            "Copilot Drone",
            "<Autocomplete Swarm>",
            &[
                paint("Skin", 0.55, 0.62, 0.72),
                paint("Shirt", 0.15, 0.15, 0.2),
                paint("Pants", 0.1, 0.1, 0.14),
                glow("Face", 0.2, 0.9, 1.0),
                paint("Hair", 0.3, 0.34, 0.4),
            ],
        ),
    ),
    (
        "scrum_master",
        v(
            CASUAL2,
            1.8,
            "Scrum Master",
            "<Stand-Up Warden>",
            &[
                paint("Shirt", 1.0, 0.55, 0.0),
                paint("Pants", 0.15, 0.2, 0.4),
                paint("Skin", 0.6, 0.42, 0.3),
            ],
        ),
    ),
    (
        "agile_coach",
        v(
            CASUAL_F,
            1.75,
            "Agile Coach",
            "<Retro Facilitator>",
            &[
                paint("Shirt", 0.15, 0.75, 0.7),
                paint("Hair", 0.9, 0.2, 0.5),
            ],
        ),
    ),
    (
        "middle_manager",
        v(
            SUIT_M,
            1.85,
            "Middle Manager",
            "<Quick Sync>",
            &[
                paint("Black", 0.4, 0.38, 0.34),
                paint("Shirt", 0.7, 0.8, 0.95),
                paint("Skin", 0.95, 0.8, 0.7),
            ],
        ),
    ),
    (
        "sales_rep",
        v(
            SUIT_F,
            1.78,
            "Sales Rep",
            "<Always Closing>",
            &[paint("Black", 0.75, 0.1, 0.15)],
        ),
    ),
    (
        "account_manager",
        v(
            SUIT_F,
            1.75,
            "Account Manager",
            "<Relationship Builder>",
            &[
                paint("Black", 0.1, 0.3, 0.45),
                paint("Skin", 0.62, 0.43, 0.3),
                paint("Hair", 0.1, 0.05, 0.03),
            ],
        ),
    ),
    (
        "linkedin_influencer",
        v(
            CASUAL_F,
            1.72,
            "LinkedIn Influencer",
            "<Thought Leader>",
            &[
                paint("Shirt", 1.0, 0.3, 0.7),
                paint("Hair", 0.98, 0.85, 0.35),
                paint("Pants", 0.95, 0.95, 0.95),
                paint("Skin", 0.9, 0.7, 0.55),
            ],
        ),
    ),
    (
        "growth_hacker",
        v(
            CASUAL2,
            1.7,
            "Growth Hacker",
            "<10x Engagement>",
            &[
                paint("Shirt", 0.1, 1.0, 0.5),
                paint("Pants", 0.1, 0.1, 0.1),
                paint("Skin", 0.7, 0.5, 0.35),
            ],
        ),
    ),
    (
        "marketing_lead",
        v(
            SUIT_F,
            2.0,
            "Head of Marketing",
            "<Brand Guardian>",
            &[paint("Black", 1.0, 0.1, 0.8), paint("Hair", 0.2, 0.9, 0.95)],
        ),
    ),
    (
        "project_manager",
        v(
            SUIT_M,
            1.8,
            "Project Manager",
            "<Gantt Zealot>",
            &[
                paint("Black", 0.1, 0.35, 0.4),
                paint("Shirt", 0.85, 0.9, 1.0),
            ],
        ),
    ),
    (
        "product_owner",
        v(
            CASUAL3,
            1.75,
            "Product Owner",
            "<Backlog Hoarder>",
            &[
                paint("Shirt", 1.0, 0.75, 0.1),
                paint("Skin", 0.45, 0.3, 0.2),
            ],
        ),
    ),
    (
        "consultant",
        v(
            WORKER,
            2.05,
            "Consultant",
            "<Billable Hours>",
            &[
                paint("Vest", 0.08, 0.12, 0.35),
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Hat", 0.95, 0.95, 0.95),
            ],
        ),
    ),
    (
        "security_analyst",
        v(
            WORKER,
            1.78,
            "Security Analyst",
            "<Clicked the Link>",
            &[
                paint("Vest", 0.1, 0.35, 0.9),
                paint("Shirt", 0.15, 0.2, 0.3),
                paint("Hat", 0.05, 0.15, 0.5),
                paint("Skin", 0.9, 0.72, 0.58),
            ],
        ),
    ),
    (
        "ciso",
        v(
            SUIT_M,
            2.1,
            "CISO",
            "<Compliance Theater>",
            &[
                paint("Black", 0.03, 0.03, 0.05),
                paint("Shirt", 0.1, 0.1, 0.12),
                paint("Skin", 0.85, 0.7, 0.6),
            ],
        ),
    ),
    (
        "ai_ethicist",
        v(DOC_F, 1.68, "AI Ethicist", "<Policy Without Teeth>", &[]),
    ),
    (
        "crypto_bro",
        v(
            CASUAL2,
            1.78,
            "Crypto Bro",
            "<To The Moon>",
            &[
                paint("Shirt", 0.95, 0.75, 0.1),
                paint("Hair", 0.95, 0.85, 0.4),
                paint("Skin", 0.9, 0.65, 0.45),
            ],
        ),
    ),
    (
        "chatbot_minion",
        v(
            CASUAL_M,
            1.25,
            "Chatbot Minion",
            "<Hallucinating Helper>",
            &[
                paint("Skin", 0.8, 0.9, 0.95),
                paint("Shirt", 0.95, 0.95, 0.98),
                glow("Face", 1.0, 0.3, 0.9),
                paint("Hair", 0.8, 0.9, 0.95),
            ],
        ),
    ),
    (
        "unreviewed_pr",
        v(
            CASUAL_M,
            1.4,
            "Unreviewed PR",
            "<LGTM>",
            &[
                paint("Shirt", 0.2, 0.8, 0.35),
                paint("Pants", 0.8, 0.2, 0.2),
                paint("Skin", 0.75, 0.85, 0.78),
                glow("Face", 0.3, 1.0, 0.5),
                paint("Hair", 0.2, 0.5, 0.3),
            ],
        ),
    ),
    (
        "hr_partner",
        v(
            SUIT_F,
            1.75,
            "HR Business Partner",
            "<People & Culture & Chatbots>",
            &[
                paint("Black", 0.95, 0.7, 0.8),
                paint("Shirt", 1.0, 1.0, 1.0),
                paint("Skin", 0.92, 0.76, 0.65),
                paint("Hair", 0.85, 0.65, 0.35),
            ],
        ),
    ),
    (
        "vibe_coder",
        v(
            CASUAL2,
            2.3,
            "The Vibe Coder",
            "<Ships Without Reading>",
            &[paint("Shirt", 0.2, 1.0, 0.4)],
        ),
    ),
    (
        "analyst",
        v(DOC_M, 2.3, "The Analyst", "<Analysis by Claude>", &[]),
    ),
    (
        "customer",
        v(
            SUIT_M,
            2.4,
            "The Customer",
            "<Was Promised This Yesterday>",
            &[
                paint("Shirt", 0.85, 0.1, 0.1),
                paint("Black", 0.2, 0.15, 0.1),
                paint("Skin", 0.9, 0.55, 0.45),
            ],
        ),
    ),
    (
        "vp_synergy",
        v(
            SUIT_F,
            2.3,
            "VP of Synergy",
            "<Reorg Enthusiast>",
            &[
                paint("Black", 0.45, 0.2, 0.7),
                paint("Skin", 0.45, 0.3, 0.22),
            ],
        ),
    ),
    (
        "ceo",
        v(SUIT_M_BALD, 2.7, "CEO", "<Chief AI Visionary>", &[]),
    ),
];

/// Every unit key the catalog knows, in table order.
pub fn unit_keys() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(k, _)| *k)
}

pub fn is_known(key: &str) -> bool {
    TABLE.iter().any(|(k, _)| *k == key)
}

/// Visual for a sim unit key. Unknown keys get a neutral casual model so a
/// content typo shows up on screen instead of crashing.
pub fn unit_visual(key: &str) -> UnitVisual {
    const FALLBACK: UnitVisual = v(CASUAL3, 1.8, "Unknown", "<Unmapped>", &[]);
    TABLE
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(FALLBACK, |(_, vis)| *vis)
}

/// Who stands where on each floor when the bridge is not driving the scene
/// (`WOW_FLOOR` debug mode). Packs are the dummies' composition.
pub fn pack_roster(floor: u8, pack: usize) -> &'static [&'static str] {
    const R: [[&[&str]; 6]; 5] = [
        [
            &["meatproxy", "meatproxy"],
            &["vibe_junior", "vibe_junior", "meatproxy"],
            &["copilot_drone", "copilot_drone", "copilot_drone"],
            &["vibe_junior", "meatproxy", "copilot_drone"],
            &["meatproxy", "vibe_junior", "vibe_junior"],
            &["copilot_drone", "copilot_drone", "meatproxy", "vibe_junior"],
        ],
        [
            &["scrum_master", "meatproxy"],
            &["agile_coach", "scrum_master", "middle_manager"],
            &["middle_manager", "meatproxy", "scrum_master"],
            &["scrum_master", "agile_coach", "meatproxy"],
            &["middle_manager", "middle_manager", "agile_coach"],
            &["hr_partner"],
        ],
        [
            &["sales_rep", "account_manager"],
            &["growth_hacker", "sales_rep", "meatproxy"],
            &["linkedin_influencer", "sales_rep", "account_manager"],
            &["growth_hacker", "growth_hacker", "meatproxy"],
            &[
                "sales_rep",
                "sales_rep",
                "account_manager",
                "linkedin_influencer",
            ],
            &["marketing_lead"],
        ],
        [
            &["project_manager", "meatproxy"],
            &["security_analyst", "security_analyst", "project_manager"],
            &["consultant", "meatproxy"],
            &["product_owner", "project_manager", "middle_manager"],
            &["security_analyst", "consultant", "product_owner"],
            &["ciso"],
        ],
        [
            &["copilot_drone", "copilot_drone", "meatproxy"],
            &["ai_ethicist", "meatproxy", "linkedin_influencer"],
            &["crypto_bro", "crypto_bro", "chatbot_minion"],
            &["ai_ethicist", "ai_ethicist", "meatproxy"],
            &["linkedin_influencer", "crypto_bro", "copilot_drone"],
            &[
                "crypto_bro",
                "ai_ethicist",
                "chatbot_minion",
                "chatbot_minion",
            ],
        ],
    ];
    R[usize::from(floor.clamp(1, 5)) - 1][pack.min(5)]
}

pub fn boss_key(floor: u8) -> &'static str {
    ["vibe_coder", "analyst", "customer", "vp_synergy", "ceo"][usize::from(floor.clamp(1, 5)) - 1]
}

/// Friendly NPC at a named anchor, with its pose.
pub fn npc_at(floor: u8, anchor: &str) -> Option<(&'static str, IdlePose)> {
    match (floor, anchor) {
        (_, "npc_1") => Some(("intern", IdlePose::Standing)),
        (1, "npc_2") => Some(("it_support", IdlePose::Sleeping)),
        (1, "npc_3") => Some(("it_support", IdlePose::Chatting)),
        (4, "npc_2") => Some(("it_support", IdlePose::Standing)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &[&str] = &[
        "senior_engineer",
        "intern",
        "meatproxy",
        "vibe_junior",
        "copilot_drone",
        "scrum_master",
        "agile_coach",
        "middle_manager",
        "sales_rep",
        "account_manager",
        "linkedin_influencer",
        "project_manager",
        "product_owner",
        "consultant",
        "ai_ethicist",
        "crypto_bro",
        "chatbot_minion",
        "unreviewed_pr",
        "vibe_coder",
        "analyst",
        "customer",
        "vp_synergy",
        "ceo",
        "hr_partner",
        "growth_hacker",
        "marketing_lead",
        "security_analyst",
        "ciso",
        "it_support",
    ];

    #[test]
    fn every_contract_key_is_mapped() {
        for k in CONTRACT {
            assert!(is_known(k), "{k} missing from the catalog");
        }
    }

    #[test]
    fn models_exist_and_stay_within_budget() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for k in unit_keys() {
            let vis = unit_visual(k);
            assert!(dir.join(vis.model).exists(), "{k}: {}", vis.model);
            assert!(CHARACTER_MODELS.contains(&vis.model), "{k}: {}", vis.model);
            assert!(!vis.title.is_empty() || k == "senior_engineer");
        }
        assert!(CHARACTER_MODELS.len() <= 11);
    }

    #[test]
    fn rosters_only_name_known_units() {
        for floor in 1..=5 {
            for pack in 0..6 {
                for k in pack_roster(floor, pack) {
                    assert!(is_known(k), "floor {floor} pack {pack}: {k}");
                }
            }
            assert!(is_known(boss_key(floor)));
        }
    }
}
