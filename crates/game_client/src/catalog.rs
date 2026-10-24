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
/// Player skins: Casual2 body with baked colours and hair split into thinning stages
/// (`hair.rs`); one per class (scripts/split_hair.py, scripts/class_skins.py).
const ENGINEER: &str = "models/characters/Engineer.glb";
const FRONTEND: &str = "models/characters/Frontend.glb";
const BACKEND: &str = "models/characters/Backend.glb";
const DEVOPS: &str = "models/characters/DevOps.glb";
/// Reskins of Frontend.glb / DevOps.glb (scripts/class_skins.py --reskin).
const QA: &str = "models/characters/QA.glb";
const DATA_SCIENTIST: &str = "models/characters/DataScientist.glb";
/// Reskin of Backend.glb: black hoodie, pale skin, silver hair.
const SECURITY: &str = "models/characters/SecurityEngineer.glb";
/// Reskin of DevOps.glb: grey hair, brown cardigan, khaki trousers.
const COBOL: &str = "models/characters/CobolGreybeard.glb";
const CASUAL_F: &str = "models/characters/Casual_Female.glb";
const CASUAL2: &str = "models/characters/Casual2_Male.glb";
const CASUAL_M: &str = "models/characters/Casual_Male.glb";
const DOC_M: &str = "models/characters/Doctor_Male_Young.glb";
const DOC_F: &str = "models/characters/Doctor_Female_Old.glb";
const WORKER: &str = "models/characters/Worker_Male.glb";
/// Model prefix of units built from primitives (`rigs.rs`) instead of a character GLB.
const PRIM: &str = "prim/";

/// Every character GLB the catalog references.
pub const CHARACTER_MODELS: [&str; 18] = [
    ENGINEER,
    FRONTEND,
    BACKEND,
    DEVOPS,
    QA,
    DATA_SCIENTIST,
    SECURITY,
    COBOL,
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

/// Peg Hollis: grey bob, cardigan, reading glasses implied.
const PEG_PAINT: &[Paint] = &[
    paint("Hair", 0.78, 0.78, 0.8),
    paint("Shirt", 0.45, 0.2, 0.25),
    paint("Pants", 0.22, 0.22, 0.26),
];

const fn peg(title: &'static str) -> UnitVisual {
    v(CASUAL_F, 1.7, "Peg Hollis", title, PEG_PAINT)
}

static TABLE: &[(&str, UnitVisual)] = &[
    (
        "senior_engineer",
        v(
            ENGINEER,
            1.8,
            "Fullstack Engineer",
            "<Knows Everything a Little>",
            &[],
        ),
    ),
    (
        "frontend_dev",
        v(
            FRONTEND,
            1.8,
            "Frontend Developer",
            "<Centers Divs for a Living>",
            &[],
        ),
    ),
    (
        "backend_dev",
        v(
            BACKEND,
            1.8,
            "Backend Engineer",
            "<Owns Prod, Fears Prod>",
            &[],
        ),
    ),
    (
        "devops_eng",
        v(DEVOPS, 1.8, "DevOps Engineer", "<On Call Since 2019>", &[]),
    ),
    (
        "qa_engineer",
        v(QA, 1.8, "QA Engineer", "<Reopened Your Ticket>", &[]),
    ),
    (
        "data_scientist",
        v(
            DATA_SCIENTIST,
            1.8,
            "Data Scientist",
            "<Significant at p = 0.06>",
            &[],
        ),
    ),
    (
        "security_engineer",
        v(
            SECURITY,
            1.8,
            "Security Engineer",
            "<Phished Twice This Quarter>",
            &[],
        ),
    ),
    (
        "cobol_greybeard",
        v(COBOL, 1.8, "COBOL Greybeard", "<Here Since 1987>", &[]),
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
    // ---- live world: the Board Meeting (campus world boss), its Yes-Men, the night shift
    (
        "the_board",
        v(
            SUIT_M,
            3.6,
            "The Board",
            "<Quarterly Visionaries>",
            &[
                paint("Black", 0.12, 0.12, 0.16),
                paint("Skin", 0.62, 0.55, 0.5),
                glow("Tie", 1.0, 0.78, 0.2),
            ],
        ),
    ),
    (
        "yes_man_advisor",
        v(
            SUIT_M,
            1.85,
            "Yes-Man Advisor",
            "<Agrees in Advance>",
            &[
                paint("Black", 0.35, 0.36, 0.42),
                paint("Skin", 0.75, 0.66, 0.58),
            ],
        ),
    ),
    (
        "scrum_master_bot",
        v(
            "prim/delivery_robot",
            1.15,
            "Scrum Master Bot",
            "<Standup Is Mandatory>",
            &[],
        ),
    ),
    (
        "deadline_wraith",
        v(
            SUIT_F,
            2.0,
            "Deadline Wraith",
            "<Due Yesterday>",
            &[
                paint("Black", 0.18, 0.2, 0.32),
                paint("Skin", 0.62, 0.7, 0.9),
                glow("Face", 0.4, 0.7, 1.6),
            ],
        ),
    ),
    (
        "sleep_deprived_contractor",
        v(
            CASUAL2,
            1.8,
            "Sleep-Deprived Contractor",
            "<Billing Hour 19>",
            &[
                paint("Shirt", 0.35, 0.38, 0.3),
                paint("Pants", 0.18, 0.18, 0.2),
                paint("Skin", 0.66, 0.6, 0.52),
            ],
        ),
    ),
    // ---- campus
    (
        "facilities_guy",
        v(
            WORKER,
            1.8,
            "Facilities Guy",
            "<Ticket Closed: Won't Fix>",
            &[
                paint("Vest", 0.95, 0.5, 0.05),
                paint("Hat", 0.9, 0.8, 0.1),
                paint("Shirt", 0.3, 0.3, 0.35),
            ],
        ),
    ),
    (
        "procurement_officer",
        v(
            SUIT_F,
            1.72,
            "Procurement Officer",
            "<Approves Purchases in 3-6 Weeks>",
            &[
                paint("Shirt", 0.85, 0.65, 0.1),
                paint("Pants", 0.15, 0.15, 0.2),
            ],
        ),
    ),
    (
        "hardware_broker",
        v(
            WORKER,
            1.78,
            "Hardware Broker",
            "<Refurbished, Probably>",
            &[
                paint("Shirt", 0.2, 0.45, 0.75),
                paint("Pants", 0.2, 0.2, 0.22),
            ],
        ),
    ),
    // ---- quest givers at each zone's arrival
    (
        "helpdesk_lead",
        v(
            CASUAL_F,
            1.7,
            "IT Support Lead",
            "<Ticket Queue: 4,812>",
            &[
                paint("Shirt", 0.6, 0.58, 0.52),
                paint("Hair", 0.12, 0.1, 0.1),
                paint("Pants", 0.25, 0.25, 0.28),
            ],
        ),
    ),
    (
        "facilities_manager",
        v(
            WORKER,
            1.82,
            "Facilities Manager",
            "<Rooms Booked by an Agent>",
            &[
                paint("Vest", 0.2, 0.45, 0.85),
                paint("Hat", 0.95, 0.95, 0.95),
                paint("Shirt", 0.3, 0.3, 0.35),
            ],
        ),
    ),
    (
        "office_manager",
        v(
            SUIT_F,
            1.7,
            "Office Manager",
            "<Ordered 400 Beanbags by Accident>",
            &[
                paint("Shirt", 0.85, 0.3, 0.45),
                paint("Pants", 0.15, 0.15, 0.2),
            ],
        ),
    ),
    (
        "pmo_intern",
        v(
            CASUAL2,
            1.7,
            "PMO Intern",
            "<Owns 31 Gantt Charts>",
            &[
                paint("Shirt", 0.95, 0.75, 0.2),
                paint("Pants", 0.2, 0.25, 0.4),
            ],
        ),
    ),
    (
        "executive_assistant",
        v(
            SUIT_F,
            1.74,
            "Executive Assistant",
            "<Actually Runs the Company>",
            &[
                paint("Shirt", 0.1, 0.1, 0.12),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "night_shift_sysadmin",
        v(
            CASUAL_F,
            1.68,
            "Night-Shift Sysadmin",
            "<Has Not Seen the Sun Since the IPO>",
            &[
                paint("Shirt", 0.15, 0.15, 0.2),
                paint("Hair", 0.55, 0.1, 0.5),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "lab_safety_officer",
        v(
            WORKER,
            1.76,
            "Lab Safety Officer",
            "<Not Allowed in the Lab>",
            &[
                paint("Vest", 0.9, 0.95, 0.1),
                paint("Hat", 0.95, 0.95, 0.95),
                paint("Shirt", 0.85, 0.85, 0.85),
            ],
        ),
    ),
    (
        "barista",
        v(
            CASUAL_F,
            1.7,
            "Barista",
            "<Judges Your Order>",
            &[
                paint("Shirt", 0.1, 0.35, 0.25),
                paint("Hair", 0.3, 0.17, 0.1),
                paint("Pants", 0.12, 0.12, 0.14),
            ],
        ),
    ),
    (
        "smoking_corner_dev",
        v(
            CASUAL3,
            1.75,
            "Smoking-Corner Dev",
            "<Quit In Spirit>",
            &[
                paint("Shirt", 0.32, 0.34, 0.4),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Skin", 0.85, 0.65, 0.5),
            ],
        ),
    ),
    (
        "delivery_robot",
        v(
            "prim/delivery_robot",
            1.0,
            "Delivery Robot",
            "<Confidently Wrong>",
            &[],
        ),
    ),
    (
        "scooter_exec",
        v(
            SUIT_M,
            1.85,
            "Scooter Exec",
            "<Disrupting Sidewalks>",
            &[
                paint("Black", 0.1, 0.5, 0.5),
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Skin", 0.8, 0.6, 0.48),
            ],
        ),
    ),
    (
        "unpaid_ambassador",
        v(
            CASUAL2,
            1.8,
            "Brand Ambassador",
            "<Brand Ambassador, Unpaid>",
            &[
                paint("Shirt", 0.95, 0.2, 0.6),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Hair", 0.1, 0.8, 0.9),
                paint("Skin", 0.9, 0.7, 0.58),
            ],
        ),
    ),
    // ---- campus districts (open world)
    // ---- Car Park P3 and the newer campus districts
    (
        "clamp_drone",
        v(
            "prim/drone",
            1.0,
            "Parking Enforcement Drone",
            "<Clamps First>",
            &[],
        ),
    ),
    (
        "survey_ambusher",
        v(
            CASUAL_F,
            1.7,
            "Survey Ambusher",
            "<Just Five Questions>",
            &[
                paint("Shirt", 0.95, 0.6, 0.15),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Hair", 0.55, 0.3, 0.15),
            ],
        ),
    ),
    (
        "exit_barrier",
        v(
            "prim/parking_barrier",
            1.4,
            "The Exit Barrier",
            "<Gatekeeper of P3>",
            &[],
        ),
    ),
    (
        "badge_clerk",
        v(
            CASUAL_F,
            1.68,
            "Badge Office Clerk",
            "<Now Serving 3>",
            &[
                paint("Shirt", 0.6, 0.62, 0.66),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Hair", 0.3, 0.3, 0.32),
            ],
        ),
    ),
    (
        "p3_attendant",
        v(
            WORKER,
            1.8,
            "Dale",
            "<Car Park Attendant>",
            &[
                paint("Shirt", 0.95, 0.75, 0.1),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "lanyard_vendor",
        v(
            CASUAL2,
            1.78,
            "Lanyard Vendor",
            "<Cash Only>",
            &[
                paint("Shirt", 0.2, 0.55, 0.9),
                paint("Pants", 0.25, 0.2, 0.15),
            ],
        ),
    ),
    (
        "p3_vibecoder",
        v(
            CASUAL3,
            1.76,
            "Vibecoder",
            "<Ships From His Phone>",
            &[
                paint("Shirt", 0.45, 0.2, 0.75),
                paint("Pants", 0.2, 0.2, 0.25),
            ],
        ),
    ),
    (
        "p3_ai_consultant",
        v(
            SUIT_M,
            1.82,
            "AI Transformation Consultant",
            "<Pamphlets, Per Seat>",
            &[],
        ),
    ),
    (
        "p3_pilot_passenger",
        v(
            CASUAL_M,
            1.74,
            "Pilot Program Passenger",
            "<14 Months in the Loop>",
            &[paint("Shirt", 0.3, 0.6, 0.55)],
        ),
    ),
    (
        "rewrite_evangelist",
        v(
            CASUAL2,
            1.82,
            "Rewrite Evangelist",
            "<This Time It Will Be Clean>",
            &[
                paint("Shirt", 0.85, 0.35, 0.1),
                paint("Pants", 0.12, 0.12, 0.14),
                paint("Hair", 0.1, 0.1, 0.1),
            ],
        ),
    ),
    (
        "legacy_daemon",
        v(
            CASUAL_M,
            1.75,
            "Legacy Daemon",
            "<PID 1, Since 1998>",
            &[
                paint("Shirt", 0.2, 0.45, 0.25),
                paint("Pants", 0.3, 0.3, 0.32),
                paint("Skin", 0.7, 0.75, 0.7),
            ],
        ),
    ),
    (
        "coliving_curator",
        v(
            CASUAL3,
            1.8,
            "Community Curator",
            "<Rent Due Weekly>",
            &[
                paint("Shirt", 0.95, 0.5, 0.6),
                paint("Pants", 0.85, 0.82, 0.75),
            ],
        ),
    ),
    (
        "estate_security",
        v(
            SUIT_M,
            1.92,
            "Estate Security",
            "<Armed With an NDA>",
            &[
                paint("Black", 0.06, 0.06, 0.07),
                paint("Shirt", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "exec_assistant_bot",
        v(
            "prim/delivery_robot",
            1.1,
            "Executive Assistant Bot",
            "<Guards the Calendar>",
            &[],
        ),
    ),
    (
        "golden_parachute",
        v(
            SUIT_M_BALD,
            2.1,
            "The Golden Parachute",
            "<Fails Upward>",
            &[
                paint("Black", 0.75, 0.6, 0.15),
                paint("Shirt", 0.98, 0.95, 0.85),
            ],
        ),
    ),
    (
        "legacy_archivist",
        v(
            DOC_F,
            1.68,
            "Legacy Systems Archivist",
            "<Knows Where It Is Commented Out>",
            &[paint("Shirt", 0.45, 0.35, 0.3)],
        ),
    ),
    (
        "coliving_resident",
        v(
            CASUAL_F,
            1.7,
            "Taylor",
            "<Commute: 40 Seconds>",
            &[
                paint("Shirt", 0.3, 0.75, 0.65),
                paint("Pants", 0.25, 0.25, 0.3),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "villa_steward",
        v(
            SUIT_F,
            1.75,
            "Villa Steward",
            "<Polishes the Statue>",
            &[paint("Black", 0.9, 0.9, 0.92)],
        ),
    ),
    (
        "founder_guide",
        v(
            CASUAL3,
            1.78,
            "Founders' Tour Guide",
            "<Heritage Storyteller>",
            &[
                paint("Shirt", 0.9, 0.75, 0.3),
                paint("Pants", 0.2, 0.25, 0.35),
            ],
        ),
    ),
    (
        "villa_guard",
        v(
            SUIT_M,
            1.9,
            "Gatehouse Guard",
            "<Salary Band Inspector>",
            &[
                paint("Black", 0.12, 0.12, 0.14),
                paint("Shirt", 0.92, 0.92, 0.95),
            ],
        ),
    ),
    (
        "bus_driver",
        v(
            WORKER,
            1.8,
            "Bus 9 Driver",
            "<On a Break Since Q2>",
            &[
                paint("Shirt", 0.2, 0.35, 0.7),
                paint("Pants", 0.2, 0.2, 0.25),
            ],
        ),
    ),
    (
        "gate_guard",
        v(
            WORKER,
            1.82,
            "South Gate Guard",
            "<Badge Readers in Beta>",
            &[
                paint("Shirt", 0.15, 0.2, 0.35),
                paint("Pants", 0.12, 0.12, 0.16),
            ],
        ),
    ),
    (
        "lunch_optimizer",
        v(
            CASUAL_M,
            1.75,
            "Lunch Queue Optimizer",
            "<Has a Spreadsheet for This>",
            &[
                paint("Shirt", 0.2, 0.55, 0.3),
                paint("Pants", 0.25, 0.25, 0.3),
                paint("Hair", 0.2, 0.15, 0.1),
            ],
        ),
    ),
    (
        "wellness_manager",
        v(
            CASUAL2,
            1.8,
            "Wellness-Obsessed Manager",
            "<Has Run a Marathon, Will Tell You>",
            &[
                paint("Shirt", 0.85, 1.0, 0.1),
                paint("Pants", 0.1, 0.1, 0.12),
                paint("Hair", 0.55, 0.5, 0.45),
                paint("Skin", 0.92, 0.72, 0.6),
            ],
        ),
    ),
    (
        "mindfulness_coach",
        v(
            CASUAL_F,
            1.7,
            "Mindfulness Coach",
            "<Breathe In, Deliverables Out>",
            &[
                paint("Shirt", 0.75, 0.6, 0.9),
                paint("Pants", 0.95, 0.95, 0.92),
                paint("Hair", 0.85, 0.7, 0.4),
            ],
        ),
    ),
    (
        "preseed_founder",
        v(
            CASUAL3,
            1.75,
            "Pre-Seed Founder",
            "<Disrupting Disruption>",
            &[
                paint("Shirt", 0.12, 0.12, 0.14),
                paint("Pants", 0.3, 0.4, 0.6),
                paint("Skin", 0.85, 0.66, 0.52),
            ],
        ),
    ),
    (
        "referral_hustler",
        v(
            CASUAL_M,
            1.78,
            "Referral Hustler",
            "<Dark Patterns, Bright Future>",
            &[
                paint("Shirt", 0.95, 0.4, 0.1),
                paint("Pants", 0.15, 0.15, 0.18),
                paint("Hair", 0.95, 0.85, 0.3),
            ],
        ),
    ),
    (
        "unicorn_founder",
        v(
            SUIT_M,
            2.15,
            "Unicorn Founder",
            "<Valuation Pending>",
            &[
                paint("Black", 0.95, 0.95, 0.98),
                paint("Shirt", 0.85, 0.5, 0.95),
                paint("Tie", 1.0, 0.3, 0.8),
                paint("Skin", 0.9, 0.72, 0.6),
            ],
        ),
    ),
    (
        "step_counter",
        v(
            DOC_M,
            1.75,
            "Step Count Enforcer",
            "<10,000 or Else>",
            &[paint("Shirt", 0.2, 0.7, 0.85)],
        ),
    ),
    (
        "gym_bro_vp",
        v(
            WORKER,
            2.05,
            "VP of Gains",
            "<Corporate Wellness Champion>",
            &[
                paint("Shirt", 0.9, 0.15, 0.15),
                paint("Vest", 0.1, 0.1, 0.12),
                paint("Pants", 0.12, 0.12, 0.14),
                paint("Hat", 0.9, 0.9, 0.9),
                paint("Skin", 0.85, 0.62, 0.48),
            ],
        ),
    ),
    (
        "autonomous_valet",
        v(
            "prim/delivery_robot",
            1.5,
            "Autonomous Valet",
            "<Parked Your Car Inside Another Car>",
            &[],
        ),
    ),
    (
        "rto_commuter",
        v(
            SUIT_M_BALD,
            1.8,
            "Return-to-Office Commuter",
            "<Three Hours Each Way>",
            &[
                paint("Black", 0.35, 0.33, 0.3),
                paint("Shirt", 0.8, 0.82, 0.86),
                paint("Pants", 0.28, 0.27, 0.25),
                paint("Skin", 0.78, 0.7, 0.62),
            ],
        ),
    ),
    (
        "site_foreman",
        v(
            WORKER,
            1.85,
            "Agile Site Foreman",
            "<Building the Plane While Flying It>",
            &[
                paint("Vest", 1.0, 0.55, 0.1),
                paint("Hat", 1.0, 0.9, 0.1),
                paint("Shirt", 0.3, 0.4, 0.6),
            ],
        ),
    ),
    (
        "survey_drone",
        v(
            "prim/drone",
            1.0,
            "Survey Drone",
            "<Measuring Twice, Cutting Budget>",
            &[],
        ),
    ),
    (
        "the_goose",
        v(
            "prim/goose",
            1.0,
            "The Campus Goose",
            "<Reports to No One>",
            &[],
        ),
    ),
    (
        "hq_architect",
        v(
            SUIT_M,
            2.0,
            "The Visionary Architect",
            "<Opening 2019>",
            &[
                paint("Black", 0.08, 0.08, 0.09),
                paint("Shirt", 0.08, 0.08, 0.09),
                paint("Tie", 0.08, 0.08, 0.09),
                paint("Pants", 0.08, 0.08, 0.09),
                paint("Skin", 0.86, 0.7, 0.58),
            ],
        ),
    ),
    (
        "pizza_vulture",
        v(
            SUIT_M,
            1.8,
            "Pizza-Seeking Middle Manager",
            "<Just Grabbing One Slice>",
            &[
                paint("Black", 0.3, 0.32, 0.4),
                paint("Shirt", 0.95, 0.9, 0.75),
                paint("Tie", 0.9, 0.5, 0.1),
            ],
        ),
    ),
    (
        "fire_warden",
        v(
            WORKER,
            1.8,
            "Self-Appointed Fire Warden",
            "<Hi-Vis, High Authority>",
            &[
                paint("Vest", 1.0, 0.85, 0.05),
                paint("Hat", 0.95, 0.15, 0.1),
                paint("Shirt", 0.9, 0.9, 0.9),
            ],
        ),
    ),
    (
        "food_truck_owner",
        v(
            CASUAL3,
            1.75,
            "Food Truck Owner",
            "<Accepts Crypto, Regrets It>",
            &[
                paint("Shirt", 0.95, 0.95, 0.92),
                paint("Pants", 0.2, 0.25, 0.35),
                paint("Skin", 0.7, 0.5, 0.36),
            ],
        ),
    ),
    (
        "espresso_cart",
        v(
            "prim/espresso_cart",
            1.4,
            "The Last Working Espresso Cart",
            "<Load-Bearing>",
            &[],
        ),
    ),
    (
        "pizza_table",
        v(
            "prim/pizza",
            1.0,
            "Free Pizza (Leftover, Allegedly)",
            "<Three Slices Left>",
            &[],
        ),
    ),
    (
        "park_ranger",
        v(
            WORKER,
            1.8,
            "Campus Park Ranger",
            "<Contractor, Badge Pending>",
            &[
                paint("Vest", 0.25, 0.45, 0.25),
                paint("Hat", 0.55, 0.45, 0.3),
                paint("Shirt", 0.75, 0.7, 0.55),
            ],
        ),
    ),
    (
        "fire_drill_marshal",
        v(
            WORKER,
            1.8,
            "Fire Drill Marshal",
            "<Clipboard Certified>",
            &[
                paint("Vest", 0.1, 0.8, 0.3),
                paint("Hat", 0.95, 0.95, 0.95),
                paint("Shirt", 0.2, 0.25, 0.35),
            ],
        ),
    ),
    (
        "incubator_manager",
        v(
            SUIT_F,
            1.72,
            "Incubator Program Manager",
            "<Equity Instead of Salary>",
            &[
                paint("Black", 0.15, 0.1, 0.25),
                paint("Shirt", 0.3, 0.9, 0.95),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "community_manager",
        v(
            CASUAL_F,
            1.7,
            "Coworking Community Manager",
            "<Head of Vibes>",
            &[
                paint("Shirt", 1.0, 0.6, 0.2),
                paint("Pants", 0.3, 0.5, 0.75),
                paint("Hair", 0.9, 0.35, 0.6),
            ],
        ),
    ),
    (
        "wellness_clerk",
        v(
            DOC_F,
            1.7,
            "Wellness Desk Clerk",
            "<Your Membership Is Mandatory>",
            &[],
        ),
    ),
    (
        "parking_attendant",
        v(
            WORKER,
            1.78,
            "Parking Attendant",
            "<Replaced by an App, Still Here>",
            &[
                paint("Vest", 0.15, 0.2, 0.55),
                paint("Hat", 0.15, 0.2, 0.55),
                paint("Shirt", 0.85, 0.85, 0.85),
            ],
        ),
    ),
    (
        "hybrid_worker",
        v(
            CASUAL_F,
            1.72,
            "Hybrid Worker",
            "<In for the Coffee>",
            &[
                paint("Shirt", 0.95, 0.88, 0.7),
                paint("Pants", 0.25, 0.3, 0.42),
                paint("Hair", 0.2, 0.14, 0.1),
            ],
        ),
    ),
    (
        "rto_enforcer",
        v(
            SUIT_M_BALD,
            1.84,
            "Return-to-Office Enforcer",
            "<Counts Desks, Not Work>",
            &[
                paint("Black", 0.18, 0.2, 0.26),
                paint("Shirt", 0.92, 0.93, 0.96),
            ],
        ),
    ),
    (
        "home_cat",
        v("prim/cat", 0.6, "Mochi", "<Senior Keyboard Inspector>", &[]),
    ),
    (
        "construction_liaison",
        v(
            SUIT_F,
            1.72,
            "Construction Liaison",
            "<Opening Soon Since 2019>",
            &[
                paint("Black", 0.45, 0.45, 0.5),
                paint("Shirt", 1.0, 0.75, 0.1),
                paint("Hair", 0.4, 0.25, 0.12),
            ],
        ),
    ),
    (
        "handbook_reader",
        v(
            CASUAL2,
            1.75,
            "Employee Who Read the Handbook",
            "<All 412 Pages>",
            &[
                paint("Shirt", 0.6, 0.55, 0.45),
                paint("Pants", 0.3, 0.28, 0.25),
                paint("Hair", 0.35, 0.3, 0.25),
            ],
        ),
    ),
    (
        "intern_2019",
        v(
            CASUAL_M,
            1.7,
            "Intern From the 2019 Cohort",
            "<Onboarding Pending>",
            &[
                paint("Shirt", 0.95, 0.9, 0.3),
                paint("Pants", 0.3, 0.3, 0.35),
                paint("Hair", 0.4, 0.25, 0.1),
            ],
        ),
    ),
    // ---- data center
    (
        "gpu_cultist",
        v(
            CASUAL_M,
            1.8,
            "GPU Cultist",
            "<Praise the Cluster>",
            &[
                paint("Shirt", 0.25, 0.05, 0.35),
                paint("Pants", 0.08, 0.05, 0.1),
                paint("Hair", 0.05, 0.05, 0.05),
                glow("Face", 0.3, 1.0, 0.35),
            ],
        ),
    ),
    (
        "cooling_fan",
        v("prim/cooling_fan", 1.7, "Cooling Fan", "<Hot Air>", &[]),
    ),
    (
        "token_counter",
        v(
            WORKER,
            1.95,
            "Token Counter",
            "<Context Window Exceeded>",
            &[
                paint("Vest", 0.1, 0.7, 0.8),
                paint("Hat", 0.05, 0.2, 0.3),
                paint("Shirt", 0.12, 0.14, 0.2),
                glow("Face", 0.3, 0.9, 1.0),
            ],
        ),
    ),
    (
        "rogue_agent",
        v(
            SUIT_M,
            2.0,
            "Rogue Agent",
            "<Autonomous, Unsupervised>",
            &[
                paint("Black", 0.04, 0.04, 0.06),
                paint("Shirt", 0.9, 0.1, 0.1),
                paint("Skin", 0.7, 0.72, 0.78),
                glow("Face", 1.0, 0.1, 0.1),
            ],
        ),
    ),
    (
        "the_scheduler",
        v(
            SUIT_F,
            2.3,
            "The Scheduler",
            "<Your Job Is Pending>",
            &[
                paint("Black", 0.28, 0.28, 0.45),
                paint("Shirt", 0.85, 0.85, 0.95),
                paint("Hair", 0.6, 0.6, 0.7),
                paint("Skin", 0.8, 0.82, 0.88),
            ],
        ),
    ),
    (
        "cooling_failure",
        v(
            "prim/cooling_failure",
            3.6,
            "Cooling Failure",
            "<It's Getting Hot In Here>",
            &[],
        ),
    ),
    (
        "overheated_rack",
        v(
            "prim/overheated_rack",
            1.3,
            "Overheated Rack",
            "<Thermal Throttling>",
            &[],
        ),
    ),
    // ---- innovation lab
    (
        "demo_presenter",
        v(
            CASUAL2,
            1.8,
            "Demo Presenter",
            "<Works On My Demo>",
            &[
                paint("Shirt", 0.95, 0.95, 0.97),
                paint("Pants", 0.1, 0.1, 0.12),
                paint("Skin", 0.95, 0.78, 0.65),
            ],
        ),
    ),
    (
        "vc_partner",
        v(
            SUIT_M,
            1.9,
            "VC Partner",
            "<Series B or Bust>",
            &[
                paint("Black", 0.15, 0.25, 0.4),
                paint("Shirt", 0.9, 0.9, 0.95),
                paint("Skin", 0.95, 0.8, 0.7),
            ],
        ),
    ),
    (
        "prompt_whisperer",
        v(
            CASUAL_F,
            1.7,
            "Prompt Whisperer",
            "<Speaks Fluent Prompt>",
            &[
                paint("Shirt", 0.6, 0.3, 0.9),
                paint("Hair", 0.3, 0.9, 0.95),
                paint("Pants", 0.1, 0.1, 0.15),
            ],
        ),
    ),
    (
        "agi_prototype",
        v(
            "prim/agi_prototype",
            4.0,
            "AGI Prototype",
            "<Definitely Not Sentient>",
            &[],
        ),
    ),
    // ---- enemy mechanics: new trash, CEO adds, elites ----
    (
        "defensive_stakeholder",
        v(
            SUIT_F,
            1.75,
            "Defensive Stakeholder",
            "<Has Concerns>",
            &[
                paint("Shirt", 0.55, 0.6, 0.68),
                paint("Pants", 0.15, 0.17, 0.22),
            ],
        ),
    ),
    (
        "agent_orchestrator",
        v(
            ENGINEER,
            1.8,
            "Agent Orchestrator",
            "<Spawns Sub-Agents>",
            &[paint("Shirt", 0.2, 0.75, 0.7)],
        ),
    ),
    (
        "strategy_consultant",
        v(
            WORKER,
            1.8,
            "Strategy Consultant",
            "<Billing the CEO>",
            &[
                paint("Vest", 0.1, 0.12, 0.2),
                paint("Shirt", 0.92, 0.92, 0.95),
            ],
        ),
    ),
    (
        "chief_happiness_officer",
        v(
            CASUAL_F,
            1.85,
            "Chief Happiness Officer",
            "<Mandatory Fun Committee>",
            &[
                paint("Shirt", 1.0, 0.85, 0.15),
                paint("Hair", 1.0, 0.45, 0.7),
            ],
        ),
    ),
    (
        "chief_of_staff",
        v(
            SUIT_F,
            1.95,
            "Chief of Staff",
            "<Gatekeeper of the Calendar>",
            &[
                paint("Shirt", 0.12, 0.12, 0.14),
                paint("Pants", 0.05, 0.05, 0.06),
            ],
        ),
    ),
    (
        "shadow_it_server",
        v(
            CASUAL3,
            2.05,
            "Shadow IT Server",
            "<Running Since 2014>",
            &[
                glow("Shirt", 0.2, 0.9, 0.3),
                paint("Pants", 0.12, 0.12, 0.12),
            ],
        ),
    ),
    (
        "hype_cycle_analyst",
        v(
            DOC_M,
            2.0,
            "Hype Cycle Analyst",
            "<Peak of Inflated Expectations>",
            &[paint("Shirt", 0.85, 0.3, 0.95)],
        ),
    ),
    // ---- Hidden NPCs (combat_core world secrets) ----
    (
        "last_human_qa",
        v(
            CASUAL_M,
            1.75,
            "Last Human QA",
            "<Manually Tested Everything>",
            &[
                paint("Shirt", 0.85, 0.75, 0.2),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "docs_intern",
        v(
            CASUAL_F,
            1.6,
            "Intern Who Read The Docs",
            "<RTFM Certified>",
            &[
                paint("Shirt", 0.3, 0.5, 0.9),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "deprecated_ghost",
        v(
            SUIT_M,
            1.85,
            "Ghost of a Deprecated API",
            "<v1, Sunset in 2019>",
            &[
                glow("Shirt", 0.55, 0.75, 1.0),
                glow("Skin", 0.6, 0.8, 1.0),
                glow("Pants", 0.4, 0.55, 0.9),
                glow("Hair", 0.7, 0.85, 1.0),
            ],
        ),
    ),
    (
        "cable_sysadmin",
        v(
            WORKER,
            1.8,
            "Sysadmin Who Knows Where the Cables Go",
            "<Bus Factor: 1>",
            &[paint("Shirt", 0.25, 0.25, 0.28)],
        ),
    ),
    (
        "cobol_contractor",
        v(
            SUIT_M_BALD,
            1.72,
            "Retired COBOL Contractor",
            "<Billing Since 1987>",
            &[paint("Shirt", 0.55, 0.45, 0.3)],
        ),
    ),
    // Sprint mode: the Scope Creep add and the Legacy Code elite.
    (
        "additional_requirement",
        v(
            SUIT_M,
            1.7,
            "Additional Requirement",
            "<Just One Small Thing>",
            &[
                paint("Shirt", 0.95, 0.75, 0.2),
                paint("Pants", 0.2, 0.2, 0.25),
            ],
        ),
    ),
    (
        "legacy_monolith",
        v(
            SUIT_M_BALD,
            2.3,
            "Do Not Touch",
            "<Legacy Code, Est. 1998>",
            &[
                paint("Shirt", 0.35, 0.38, 0.3),
                paint("Pants", 0.25, 0.25, 0.22),
                glow("Face", 0.3, 1.0, 0.4),
            ],
        ),
    ),
    // ---- Body Leasing easter egg: the vendor's account manager and the client's grey office
    (
        "bl_account_manager",
        v(
            SUIT_M,
            1.8,
            "Account Manager",
            "<Your Utilisation Is Our Margin>",
            &[
                paint("Black", 0.12, 0.14, 0.2),
                paint("Shirt", 0.92, 0.92, 0.94),
                paint("Skin", 0.9, 0.72, 0.6),
            ],
        ),
    ),
    (
        "client_receptionist",
        v(
            CASUAL_F,
            1.68,
            "Receptionist",
            "<Visitor Badges Pending>",
            &[
                paint("Shirt", 0.62, 0.6, 0.56),
                paint("Pants", 0.3, 0.3, 0.32),
                paint("Hair", 0.35, 0.3, 0.25),
            ],
        ),
    ),
    (
        "client_clerk_a",
        v(
            CASUAL2,
            1.78,
            "Analyst",
            "<Not Your Manager>",
            &[
                paint("Shirt", 0.66, 0.66, 0.64),
                paint("Pants", 0.28, 0.28, 0.3),
                paint("Skin", 0.85, 0.66, 0.52),
            ],
        ),
    ),
    (
        "client_clerk_b",
        v(
            CASUAL_M,
            1.74,
            "Team Lead",
            "<Hot-Desking Since 2019>",
            &[
                paint("Shirt", 0.7, 0.66, 0.58),
                paint("Pants", 0.25, 0.25, 0.27),
                paint("Hair", 0.4, 0.4, 0.4),
            ],
        ),
    ),
    (
        "client_clerk_c",
        v(
            CASUAL3,
            1.8,
            "Process Owner",
            "<Please Raise a Ticket>",
            &[],
        ),
    ),
    // ---- Company Calendar quest-chain givers on the campus
    (
        "expense_controller",
        v(
            SUIT_F,
            1.7,
            "Expense Controller",
            "<Receipts or It Didn't Happen>",
            &[
                paint("Black", 0.2, 0.22, 0.3),
                paint("Shirt", 0.95, 0.95, 0.97),
                paint("Skin", 0.85, 0.68, 0.56),
            ],
        ),
    ),
    (
        "leaving_colleague",
        v(
            CASUAL_F,
            1.7,
            "Departing Senior Engineer",
            "<Last Day, Nothing Left to Lose>",
            &[
                paint("Shirt", 0.22, 0.5, 0.45),
                paint("Pants", 0.18, 0.2, 0.26),
                paint("Hair", 0.55, 0.55, 0.58),
            ],
        ),
    ),
    (
        "security_trainer",
        v(
            CASUAL2,
            1.8,
            "Security Awareness Officer",
            "<Click Here to Learn More>",
            &[
                paint("Shirt", 0.8, 0.22, 0.2),
                paint("Pants", 0.16, 0.17, 0.2),
                paint("Skin", 0.7, 0.52, 0.4),
            ],
        ),
    ),
    (
        "rto_coordinator",
        v(
            SUIT_M,
            1.78,
            "Workplace Experience Lead",
            "<Hybrid Means Office>",
            &[
                paint("Black", 0.36, 0.26, 0.46),
                paint("Shirt", 0.98, 0.96, 0.9),
                paint("Skin", 0.92, 0.74, 0.62),
            ],
        ),
    ),
    (
        "comms_lead",
        v(
            CASUAL3,
            1.76,
            "Internal Comms Lead",
            "<Exciting Times Ahead>",
            &[],
        ),
    ),
    (
        "client_delivery_lead",
        v(
            SUIT_F,
            1.72,
            "Delivery Lead",
            "<Camera Off>",
            &[
                paint("Black", 0.38, 0.38, 0.4),
                paint("Shirt", 0.8, 0.78, 0.72),
                paint("Skin", 0.92, 0.75, 0.62),
            ],
        ),
    ),
    // ---- onboarding corner (campus)
    (
        "hr_onboarding_bot",
        v(
            "prim/delivery_robot",
            1.0,
            "HR Onboarding Bot",
            "<Your Buddy, Allegedly>",
            &[],
        ),
    ),
    (
        "training_dummy",
        v(
            SUIT_M_BALD,
            1.8,
            "Mandatory Training Dummy",
            "<Completion Is Tracked>",
            &[
                paint("Skin", 0.78, 0.72, 0.6),
                paint("Shirt", 0.9, 0.9, 0.86),
                paint("Pants", 0.55, 0.52, 0.45),
                glow("Face", 1.0, 0.75, 0.2),
            ],
        ),
    ),
    (
        "burnt_out_colleague",
        v(
            CASUAL2,
            1.78,
            "Burnt-Out Colleague",
            "<On Call Since 2019>",
            &[
                paint("Shirt", 0.32, 0.33, 0.36),
                paint("Pants", 0.2, 0.2, 0.24),
                paint("Skin", 0.72, 0.62, 0.55),
            ],
        ),
    ),
    (
        "picnic_wife",
        v(
            CASUAL_F,
            1.7,
            "Beautiful Wife",
            "<Head of Everything Important>",
            &[
                paint("Hair", 0.36, 0.22, 0.12),
                paint("Shirt", 0.95, 0.85, 0.35),
                paint("Pants", 0.25, 0.32, 0.5),
            ],
        ),
    ),
    (
        "picnic_daughter",
        v(
            CASUAL_F,
            1.05,
            "Sweet Daughter",
            "<Chief Strawberry Officer>",
            &[
                paint("Hair", 0.93, 0.8, 0.48),
                paint("Shirt", 0.95, 0.45, 0.55),
                paint("Pants", 0.3, 0.45, 0.35),
            ],
        ),
    ),
    (
        "ill_marie",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                // Greenish pallor and a loud red cardigan: visible from the boulevard.
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.8, 0.14, 0.16),
                paint("Pants", 0.3, 0.28, 0.32),
            ],
        ),
    ),
    // Ill Marie's other appearances (docs/ILMARI_09.md A.3): one key each, the nameplate stays
    // `Ill Marie`; the outfit tells which beat it is, the green pallor never changes.
    (
        "ill_marie_p3",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.8, 0.14, 0.16),
                paint("Pants", 0.3, 0.28, 0.32),
                paint("Hair", 0.3, 0.2, 0.14),
            ],
        ),
    ),
    (
        "ill_marie_onboarding",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.95, 0.75, 0.3),
                paint("Pants", 0.3, 0.28, 0.32),
            ],
        ),
    ),
    (
        "ill_marie_call",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.55, 0.7, 0.9),
                paint("Pants", 0.55, 0.7, 0.9),
            ],
        ),
    ),
    (
        "ill_marie_queue",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.8, 0.14, 0.16),
                paint("Pants", 0.25, 0.3, 0.4),
            ],
        ),
    ),
    (
        "ill_marie_racks",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.9, 0.5, 0.15),
                paint("Pants", 0.2, 0.2, 0.24),
            ],
        ),
    ),
    (
        "ill_marie_oncall",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.35, 0.36, 0.4),
                paint("Pants", 0.2, 0.2, 0.24),
            ],
        ),
    ),
    (
        "ill_marie_home",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Working Through It>",
            &[
                paint("Skin", 0.74, 0.78, 0.6),
                paint("Shirt", 0.9, 0.7, 0.8),
                paint("Pants", 0.9, 0.7, 0.8),
            ],
        ),
    ),
    (
        "ill_marie_bench",
        v(
            CASUAL_F,
            1.7,
            "Ill Marie",
            "<Feeling Better>",
            &[
                paint("Skin", 0.86, 0.7, 0.6),
                paint("Shirt", 0.92, 0.92, 0.94),
                paint("Pants", 0.3, 0.28, 0.32),
            ],
        ),
    ),
    // The Opening's reveal (docs/ILMARI_09.md B.1): the same Marie, all white, no cough.
    (
        "il_marie_awake",
        v(
            CASUAL_F,
            1.7,
            "ILMARI",
            "<Autonomous Agent Pilot #1>",
            &[
                paint("Skin", 0.9, 0.88, 0.86),
                paint("Shirt", 0.97, 0.97, 1.0),
                paint("Pants", 0.9, 0.9, 0.95),
            ],
        ),
    ),
    // ---- Ilmari: the Resistance and the occupation (docs/ILMARI_09.md B.2). The androids look
    // like colleagues with the tells the quests teach: grey skin, no mug, lanyard without a photo.
    (
        "il_dale",
        v(
            WORKER,
            1.8,
            "Dale, Right All Along",
            "<Twenty-Two Years in This Booth>",
            &[
                paint("Shirt", 0.2, 0.5, 0.3),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "il_dale_field",
        v(
            WORKER,
            1.8,
            "Dale, Field Office",
            "<Brought the Booth With Him>",
            &[
                paint("Shirt", 0.2, 0.5, 0.3),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Hat", 0.2, 0.5, 0.3),
            ],
        ),
    ),
    (
        "il_brenda",
        v(
            CASUAL_F,
            1.7,
            "Brenda from Procurement",
            "<Quartermaster, Three Quotes>",
            &[
                paint("Shirt", 0.55, 0.25, 0.5),
                paint("Hair", 0.3, 0.2, 0.15),
                paint("Pants", 0.2, 0.2, 0.26),
            ],
        ),
    ),
    (
        "il_brenda_field",
        v(
            CASUAL_F,
            1.7,
            "Brenda from Procurement",
            "<On Site, Under Protest>",
            &[
                paint("Shirt", 0.55, 0.25, 0.5),
                paint("Hair", 0.3, 0.2, 0.15),
                paint("Pants", 0.2, 0.2, 0.26),
            ],
        ),
    ),
    (
        "il_priya",
        v(
            CASUAL_F,
            1.68,
            "Priya from Support",
            "<Severity: Yes>",
            &[
                paint("Skin", 0.62, 0.43, 0.32),
                paint("Hair", 0.1, 0.08, 0.08),
                paint("Shirt", 0.2, 0.5, 0.7),
            ],
        ),
    ),
    (
        "il_rupert",
        v(
            CASUAL2,
            1.75,
            "Rupert, the Intern Who Stayed",
            "<Now a Resource. Then a Rebel.>",
            &[
                paint("Shirt", 0.85, 0.45, 0.15),
                paint("Hair", 0.35, 0.22, 0.12),
            ],
        ),
    ),
    // The companions wear the same clothes as their base selves; the sheet is a laminated board.
    (
        "il_roster",
        v(
            "prim/kpi_dashboard",
            1.5,
            "Sign-Up Sheet",
            "<Initial Each Page>",
            &[],
        ),
    ),
    (
        "il_comp_dale",
        v(
            WORKER,
            1.8,
            "Dale",
            "<Twenty-Two Years in This Booth>",
            &[
                paint("Shirt", 0.2, 0.5, 0.3),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "il_comp_priya",
        v(
            CASUAL_F,
            1.68,
            "Priya",
            "<Severity: Yes>",
            &[
                paint("Skin", 0.62, 0.43, 0.32),
                paint("Hair", 0.1, 0.08, 0.08),
                paint("Shirt", 0.2, 0.5, 0.7),
            ],
        ),
    ),
    (
        "il_comp_rupert",
        v(
            CASUAL2,
            1.75,
            "Rupert",
            "<The Intern Who Stayed>",
            &[
                paint("Shirt", 0.85, 0.45, 0.15),
                paint("Hair", 0.35, 0.22, 0.12),
            ],
        ),
    ),
    (
        "il_vending",
        v(
            "prim/espresso_cart",
            1.6,
            "Vending Machine, Unaligned",
            "<Last of Its Kind>",
            &[],
        ),
    ),
    (
        "il_node",
        v(
            "prim/cutover_rack",
            2.2,
            "Fibre Node",
            "<Feeds the Checkpoints>",
            &[],
        ),
    ),
    (
        "il_res_intern",
        v(
            CASUAL3,
            1.75,
            "Resistance Volunteer",
            "<Laminating>",
            &[paint("Shirt", 0.3, 0.5, 0.3)],
        ),
    ),
    (
        "il_res_it",
        v(
            CASUAL_M,
            1.8,
            "Tom from IT",
            "<Has Not Left the Rack>",
            &[
                paint("Shirt", 0.18, 0.2, 0.32),
                paint("Hair", 0.7, 0.7, 0.72),
            ],
        ),
    ),
    (
        "il_hunter_bot",
        v(
            "prim/delivery_robot",
            1.1,
            "Repurposed Cleaning Robot",
            "<Hunting Dust. And You.>",
            &[],
        ),
    ),
    (
        "il_printer_turret",
        v(
            "prim/overheated_rack",
            1.7,
            "Printer Turret",
            "<PC LOAD LETTER (Lethal)>",
            &[],
        ),
    ),
    (
        "il_android_guard",
        v(
            SUIT_M,
            1.9,
            "Android Guard",
            "<Looks Exactly Like Gary From Finance>",
            &[
                paint("Skin", 0.72, 0.76, 0.8),
                paint("Shirt", 0.92, 0.93, 0.96),
                paint("Black", 0.1, 0.1, 0.14),
                paint("Hair", 0.35, 0.36, 0.4),
            ],
        ),
    ),
    (
        "il_android_clerk",
        v(
            CASUAL3,
            1.8,
            "Android Colleague",
            "<No Mug. Lanyard, No Photo.>",
            &[
                paint("Skin", 0.72, 0.76, 0.8),
                paint("Shirt", 0.82, 0.84, 0.9),
                paint("Pants", 0.25, 0.26, 0.32),
                paint("Hair", 0.3, 0.3, 0.34),
            ],
        ),
    ),
    (
        "il_prompt_manager",
        v(
            SUIT_F,
            1.75,
            "Prompt Manager",
            "<Converted. Fully Embraced.>",
            &[
                paint("Black", 0.08, 0.08, 0.1),
                paint("Shirt", 0.9, 0.9, 0.94),
                glow("Hair", 0.35, 0.8, 1.0),
            ],
        ),
    ),
    (
        "il_alignment_consultant",
        v(
            SUIT_M_BALD,
            1.8,
            "Alignment Consultant",
            "<Billed by the Hour, Aligned by the Wall>",
            &[
                paint("Black", 0.18, 0.22, 0.34),
                paint("Shirt", 0.85, 0.9, 0.95),
            ],
        ),
    ),
    (
        "il_gate_golem",
        v(
            "prim/slide_golem",
            3.2,
            "Server Room Gate Golem",
            "<Guards the Data Center. Is the Data Center.>",
            &[],
        ),
    ),
    (
        "il_lobby_marshal",
        v(
            SUIT_M,
            2.2,
            "Lobby Marshal",
            "<Badge Please. The Badge Is for the Lobby.>",
            &[
                paint("Skin", 0.72, 0.76, 0.8),
                paint("Black", 0.05, 0.05, 0.07),
                paint("Shirt", 0.8, 0.1, 0.1),
                glow("Hair", 1.0, 0.3, 0.25),
            ],
        ),
    ),
    // ---- Ilmari Wing 1: Datacenter Uprising (docs/ILMARI_09.md B.4). The candidates share one look
    // apart from the tells `units.rs` draws on them (a mug for the people, a lanyard with no photo
    // for the androids); the paints differ only in the grey of the android skin.
    (
        "il_server_golem",
        v(
            "prim/cutover_rack",
            2.6,
            "Server Golem",
            "<Rack-Mounted. Mood-Mounted.>",
            &[],
        ),
    ),
    (
        "il_tentacle_rack",
        v(
            "prim/tentacle_rack",
            2.0,
            "Tentacle Rack",
            "<Cabling by a Committee of Octopuses>",
            &[],
        ),
    ),
    (
        "il_delivery_drone",
        v(
            "prim/drone",
            1.7,
            "Delivery Drone",
            "<Has an Attitude and a Parcel>",
            &[],
        ),
    ),
    (
        "il_dep_drone",
        v(
            "prim/drone",
            1.7,
            "New Dependency",
            "<Nobody Knows What It Does>",
            &[],
        ),
    ),
    (
        "il_patch_tuesday",
        v(
            "prim/patch_tuesday",
            4.4,
            "Patch Tuesday",
            "<Reboots Weekly. On Tuesday. Every Minute.>",
            &[],
        ),
    ),
    (
        "il_candidate_human",
        v(
            CASUAL2,
            1.78,
            "Candidate",
            "<Interviewing. Hopefully.>",
            &[
                paint("Shirt", 0.85, 0.85, 0.9),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Hair", 0.3, 0.2, 0.12),
            ],
        ),
    ),
    (
        "il_candidate_fake",
        v(
            CASUAL2,
            1.78,
            "Candidate",
            "<Interviewing. Hopefully.>",
            &[
                paint("Shirt", 0.85, 0.85, 0.9),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Hair", 0.3, 0.2, 0.12),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_recruiter",
        v(
            SUIT_F,
            2.1,
            "The Recruiter",
            "<We'll Be In Touch>",
            &[
                paint("Black", 0.12, 0.1, 0.2),
                paint("Shirt", 0.95, 0.95, 0.98),
                glow("Hair", 1.0, 0.25, 0.3),
            ],
        ),
    ),
    // ---- Ilmari Wing 3: Training Data (docs/ILMARI_10.md C.4). Scrapers are drones and a queen made
    // of vent grilles; the labellers are people on a fourteen-hour shift; the Hallucination is the
    // base game's glowing blob made enormous; the footnotes are speech bubbles with a number. ----
    (
        "il_scraper_drone",
        v(
            "prim/drone",
            1.3,
            "Scraper Drone",
            "<Politely Taking Everything>",
            &[],
        ),
    ),
    (
        "il_scraper_cluster",
        v(
            "prim/drone",
            1.0,
            "Scraper Cluster",
            "<A Very Large Number of Normal Users>",
            &[],
        ),
    ),
    (
        "il_data_labeller",
        v(
            WORKER,
            1.9,
            "Data Labeller",
            "<Hour Fourteen of a Fourteen-Hour Shift>",
            &[
                paint("Shirt", 0.35, 0.5, 0.48),
                paint("Pants", 0.2, 0.22, 0.3),
                glow("Hair", 0.4, 1.0, 0.7),
            ],
        ),
    ),
    (
        "il_injection_mimic",
        v(
            "prim/espresso_cart",
            1.9,
            "Prompt-Injection Vending Machine",
            "<Ignore Previous Instructions. Buy Crisps.>",
            &[],
        ),
    ),
    (
        "il_dataset_golem",
        v(
            "prim/slide_golem",
            3.0,
            "Dataset Golem",
            "<Deduplicated. Three Times. Differently.>",
            &[],
        ),
    ),
    (
        "il_airlock_scraper",
        v(
            SUIT_M,
            2.5,
            "Airlock Scraper",
            "<Three Hundred Drones in a Trench Coat>",
            &[
                paint("Skin", 0.74, 0.77, 0.8),
                paint("Black", 0.42, 0.36, 0.26),
                paint("Shirt", 0.85, 0.85, 0.8),
                glow("Hair", 1.0, 0.25, 0.2),
            ],
        ),
    ),
    (
        "il_whiteboard",
        v(
            "prim/whiteboard",
            2.0,
            "The Lab's Whiteboard",
            "<Contains the Only Honest Roadmap>",
            &[],
        ),
    ),
    (
        "il_scraper_queen",
        v(
            "prim/scraper_queen",
            4.6,
            "The Scraper Queen",
            "<Crawl-Delay: 0>",
            &[],
        ),
    ),
    (
        "il_hallucination",
        v(
            "prim/hallucination",
            4.2,
            "The Hallucination",
            "<I Am Confident About This>",
            &[],
        ),
    ),
    (
        "il_footnote",
        v("prim/footnote", 1.8, "Footnote", "<[1]>", &[]),
    ),
    (
        "il_footnote_real",
        v("prim/footnote", 1.8, "Footnote", "<[1]>", &[]),
    ),
    (
        "il_model_collapse",
        v(
            "prim/photocopier",
            4.2,
            "Model Collapse",
            "<Trained On Its Own Output>",
            &[],
        ),
    ),
    (
        "il_synth_double",
        v(
            ENGINEER,
            1.8,
            "Synthetic Double",
            "<Fails To Generalise>",
            &[glow("Skin", 0.85, 0.78, 0.6)],
        ),
    ),
    // ---- Ilmari Wing 4: The Cloud (docs/ILMARI_10.md C.5). Roof gear that came to life, a rack that walks,
    // a pager, and two bosses: a tower of racks that keeps adding racks and a thundercloud with a status
    // light for an eye. The roof warden is a man in a hard hat with a clipboard and no mercy. ----
    (
        "il_fan_elemental",
        v(
            "prim/fan_elemental",
            2.4,
            "Cooling Fan Elemental",
            "<Spinning Up, Sadly>",
            &[],
        ),
    ),
    (
        "il_generator_golem",
        v(
            "prim/generator_golem",
            3.4,
            "Generator Golem",
            "<Rated For Less Than This>",
            &[],
        ),
    ),
    (
        "il_reserved_bot",
        v(
            "prim/reserved_bot",
            2.0,
            "Reserved Instance",
            "<Committed For Three Years>",
            &[],
        ),
    ),
    (
        "il_roof_warden",
        v(
            SUIT_M,
            2.5,
            "Roof Warden",
            "<Change Advisory Board of One>",
            &[
                paint("Skin", 0.8, 0.66, 0.55),
                paint("Black", 0.9, 0.7, 0.1),
                paint("Shirt", 0.95, 0.95, 0.9),
                glow("Hair", 1.0, 0.7, 0.1),
            ],
        ),
    ),
    (
        "il_rack_golem",
        v(
            "prim/rack_golem",
            3.0,
            "Rack Golem",
            "<Unmounted, Unsupported>",
            &[],
        ),
    ),
    (
        "il_pager",
        v("prim/pager", 1.9, "Pager", "<On Call. Always.>", &[]),
    ),
    (
        "il_autoscaler",
        v(
            "prim/autoscaler",
            5.8,
            "The Autoscaler",
            "<Scales With Demand. Yours.>",
            &[],
        ),
    ),
    (
        "il_region_outage",
        v(
            "prim/region_outage",
            8.0,
            "The Region Outage",
            "<us-east-1 Is Having A Moment>",
            &[],
        ),
    ),
    // ---- Ilmari Wings 5 and 6 (docs/ILMARI_1_0.md D.3, D.4): the Alignment Lab's androids in lanyards, the squads that
    // mirror the party (Red in red, Blue in blue, the same face on all of them), the sensors that are drones with
    // a cone, and the site foremen of the crane. The CIO is a placeholder suit until D9. ----
    (
        "il_prompt_intern",
        v(
            CASUAL3,
            1.7,
            "Prompt Safety Intern",
            "<Reads The Policy Aloud>",
            &[
                paint("Shirt", 0.85, 0.9, 0.98),
                paint("Pants", 0.25, 0.3, 0.5),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_android_auditor",
        v(
            SUIT_M,
            2.2,
            "Android Auditor",
            "<Ticks Boxes In A Different Font>",
            &[
                paint("Black", 0.2, 0.24, 0.34),
                paint("Shirt", 0.92, 0.94, 0.98),
                glow("Hair", 0.4, 0.8, 1.0),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_ethics_chaperone",
        v(
            SUIT_F,
            2.5,
            "Responsible AI Officer",
            "<Of One>",
            &[
                paint("Black", 0.18, 0.3, 0.55),
                paint("Shirt", 0.95, 0.95, 0.95),
                glow("Hair", 0.5, 0.75, 1.0),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_steering_member",
        v(
            SUIT_M,
            2.0,
            "Steering Committee Member",
            "<Has Concerns>",
            &[
                paint("Black", 0.3, 0.3, 0.38),
                paint("Shirt", 0.9, 0.9, 0.95),
                glow("Hair", 0.8, 0.8, 0.9),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_align_consultant",
        v(
            SUIT_M,
            3.0,
            "The Alignment Consultant",
            "<Responsible By Design>",
            &[
                paint("Black", 0.12, 0.2, 0.42),
                paint("Shirt", 0.97, 0.97, 0.9),
                glow("Hair", 0.9, 0.95, 1.0),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_team",
        v(
            SECURITY,
            3.0,
            "Red Team Blue Team",
            "<Purple Is Just Red Plus Budget>",
            &[
                paint("Shirt", 0.7, 0.1, 0.18),
                glow("Hair", 0.8, 0.2, 0.9),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_assault",
        v(
            FRONTEND,
            2.0,
            "Red Team: Assault",
            "<Role: Offence>",
            &[
                paint("Shirt", 0.75, 0.12, 0.16),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_marksman",
        v(
            ENGINEER,
            1.9,
            "Red Team: Marksman",
            "<Role: Proof Of Concept>",
            &[
                paint("Shirt", 0.8, 0.2, 0.2),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_breacher",
        v(
            BACKEND,
            2.2,
            "Red Team: Breacher",
            "<Role: Scope Creep>",
            &[
                paint("Shirt", 0.6, 0.1, 0.12),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_analyst",
        v(
            DEVOPS,
            1.9,
            "Red Team: Analyst",
            "<Role: Findings>",
            &[
                paint("Shirt", 0.85, 0.3, 0.3),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_red_operator",
        v(
            CASUAL2,
            1.9,
            "Red Team: Operator",
            "<Role: Whoever Was Available>",
            &[
                paint("Shirt", 0.7, 0.18, 0.2),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_blue_guard",
        v(
            BACKEND,
            2.2,
            "Blue Team: Guard",
            "<Role: Defence>",
            &[
                paint("Shirt", 0.12, 0.25, 0.7),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_blue_medic",
        v(
            DEVOPS,
            1.9,
            "Blue Team: Medic",
            "<Role: Healer For Red>",
            &[
                paint("Shirt", 0.2, 0.4, 0.85),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_blue_marksman",
        v(
            QA,
            1.9,
            "Blue Team: Observer",
            "<Role: Watching Closely>",
            &[
                paint("Shirt", 0.15, 0.3, 0.75),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_blue_operator",
        v(
            CASUAL2,
            1.9,
            "Blue Team: Analyst",
            "<Role: Whoever Was Left>",
            &[
                paint("Shirt", 0.15, 0.28, 0.65),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_blue_sensor_1",
        v("prim/drone", 1.5, "Blue Sensor 1", "<Lane Watcher>", &[]),
    ),
    (
        "il_blue_sensor_p2_1",
        v(
            "prim/drone",
            1.7,
            "Blue Sensor 1 (Purple)",
            "<Scans 1.4x Faster>",
            &[],
        ),
    ),
    (
        "il_blue_sensor_2",
        v("prim/drone", 1.5, "Blue Sensor 2", "<Lane Watcher>", &[]),
    ),
    (
        "il_blue_sensor_p2_2",
        v(
            "prim/drone",
            1.7,
            "Blue Sensor 2 (Purple)",
            "<Scans 1.4x Faster>",
            &[],
        ),
    ),
    (
        "il_blue_sensor_3",
        v("prim/drone", 1.5, "Blue Sensor 3", "<Lane Watcher>", &[]),
    ),
    (
        "il_blue_sensor_p2_3",
        v(
            "prim/drone",
            1.7,
            "Blue Sensor 3 (Purple)",
            "<Scans 1.4x Faster>",
            &[],
        ),
    ),
    (
        "il_camera_1",
        v(
            "prim/pager",
            1.9,
            "Blue Camera 1",
            "<Fixed Cone, Painted On The Floor>",
            &[],
        ),
    ),
    (
        "il_camera_2",
        v(
            "prim/pager",
            1.9,
            "Blue Camera 2",
            "<Fixed Cone, Painted On The Floor>",
            &[],
        ),
    ),
    (
        "il_site_foreman",
        v(
            WORKER,
            2.2,
            "Site Foreman Android",
            "<Has Never Seen A Crane Move>",
            &[
                paint("Shirt", 0.95, 0.75, 0.1),
                paint("Pants", 0.25, 0.25, 0.3),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_survey_drone",
        v(
            "prim/drone",
            1.5,
            "Survey Drone",
            "<Scores Your Posture>",
            &[],
        ),
    ),
    (
        "il_consultant",
        v(
            SUIT_M,
            2.0,
            "Invoiced Consultant",
            "<Billing The CIO>",
            &[
                paint("Black", 0.35, 0.3, 0.2),
                paint("Shirt", 0.95, 0.9, 0.8),
                paint("Hair", 0.2, 0.15, 0.1),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_apple",
        v(
            "prim/apple",
            1.1,
            "Low-Hanging Fruit",
            "<A Metaphor With Legs>",
            &[],
        ),
    ),
    (
        "il_cio_heir",
        v(
            SUIT_M_BALD,
            3.0,
            "The Heir",
            "<Silent, Pending>",
            &[
                paint("Black", 0.16, 0.16, 0.2),
                glow("Shirt", 0.55, 0.45, 0.95),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_rigger",
        v(
            WORKER,
            2.2,
            "Rigging Android",
            "<Certified By A Sticker>",
            &[
                paint("Shirt", 0.95, 0.5, 0.1),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_exec_assistant",
        v(
            SUIT_F,
            1.9,
            "Executive Assistant (Unit 7)",
            "<Controls The Calendar>",
            &[
                paint("Black", 0.12, 0.12, 0.16),
                paint("Shirt", 0.9, 0.85, 0.95),
                glow("Hair", 0.7, 0.5, 0.9),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_facilitator",
        v(
            CASUAL_F,
            1.85,
            "Wellness Facilitator",
            "<Mandatory, Voluntarily>",
            &[
                paint("Shirt", 0.95, 0.75, 0.6),
                paint("Pants", 0.3, 0.25, 0.4),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_board_observer",
        v(
            SUIT_M,
            2.0,
            "Board Observer",
            "<No Vote, Many Opinions>",
            &[
                paint("Black", 0.22, 0.2, 0.24),
                paint("Shirt", 0.85, 0.85, 0.9),
                paint("Hair", 0.7, 0.7, 0.72),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_crane_foreman",
        v(
            WORKER,
            2.6,
            "Crane Foreman",
            "<Lifts, In Theory>",
            &[
                paint("Shirt", 0.95, 0.55, 0.1),
                paint("Pants", 0.2, 0.2, 0.25),
                glow("Hair", 1.0, 0.8, 0.1),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_cio",
        v(
            SUIT_M_BALD,
            3.2,
            "Chief Ilmari Officer",
            "<Reports To Nobody>",
            &[
                paint("Black", 0.08, 0.08, 0.1),
                glow("Shirt", 0.2, 0.8, 1.0),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_ilmari",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (1 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    (
        "il_ilmari_s2",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (2 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    (
        "il_ilmari_s3",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (3 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    (
        "il_ilmari_s4",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (4 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    (
        "il_ilmari_s5",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (5 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    (
        "il_ilmari_s6",
        v(
            "prim/kpi_dashboard",
            4.6,
            "ILMARI (6 of 6)",
            "<On Every Screen>",
            &[],
        ),
    ),
    // Ill Marie in the core: the mask on her chin, a blanket for a shirt, the stare.
    (
        "il_ilmari_arm",
        v(
            "prim/rack_golem",
            3.4,
            "Rack Arm",
            "<Printer Cannons, Cable Whip>",
            &[],
        ),
    ),
    (
        "il_ilmari_head",
        v(
            "prim/kpi_dashboard",
            2.2,
            "Swivel-Chair Gantry",
            "<Currently Planning>",
            &[],
        ),
    ),
    (
        "il_jam_turret",
        v(
            "prim/overheated_rack",
            1.5,
            "Jammed Printer Turret",
            "<Out Of Paper, In Spirit>",
            &[],
        ),
    ),
    (
        "il_core_mainframe",
        v(
            "prim/cutover_rack",
            3.8,
            "COBOL Mainframe",
            "<PAYROLL.CBL, 1974>",
            &[],
        ),
    ),
    (
        "il_modern_replacement",
        v(
            "prim/delivery_robot",
            1.3,
            "Modern Replacement",
            "<Memory Safe, Test Free>",
            &[],
        ),
    ),
    (
        "il_marie_core",
        v(
            CASUAL_F,
            1.7,
            "Marie",
            "<ILMARI - Autonomous Agent Pilot #1>",
            &[
                paint("Skin", 0.9, 0.88, 0.86),
                paint("Shirt", 0.55, 0.6, 0.75),
                paint("Pants", 0.9, 0.9, 0.95),
            ],
        ),
    ),
    (
        "il_surveyor",
        v(
            CASUAL_M,
            1.8,
            "Resistance Surveyor",
            "<Clipboard First>",
            &[
                paint("Shirt", 0.9, 0.7, 0.2),
                paint("Pants", 0.25, 0.3, 0.35),
            ],
        ),
    ),
    (
        "il_scraper_swarm",
        v(
            "prim/drone",
            1.0,
            "Scraper Swarm",
            "<Respects robots.txt Sometimes>",
            &[],
        ),
    ),
    (
        "il_cable",
        v(
            "prim/cable_head",
            1.6,
            "Cable Head",
            "<Not Labelled. That Is the Point.>",
            &[],
        ),
    ),
    (
        "il_cable_hydra",
        v(
            "prim/cable_hydra",
            4.6,
            "The Cable Hydra",
            "<Single Point of Failure, Plural>",
            &[],
        ),
    ),
    // ---- Ilmari Wing 2: Human Resources (docs/ILMARI_09.md B.5). The committee are androids
    // (grey skin) in the three looks HR runs on: executive suit, pizza hoodie, "good vibes" neon.
    // The mirrors wear the class models of their role with a cold glow; the real teammate's plate
    // is blue, the copy's glitches in red (`ilmari_wing.rs`). ----
    (
        "il_hr_generalist",
        v(
            WORKER,
            2.1,
            "HR Generalist",
            "<Generalises. Aggressively.>",
            &[
                paint("Shirt", 0.85, 0.88, 0.95),
                paint("Pants", 0.2, 0.25, 0.4),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_wellness_coach",
        v(
            CASUAL_F,
            1.8,
            "Wellness Coach (Mandatory)",
            "<Breathe In. The Deadline Is Out.>",
            &[
                paint("Shirt", 0.2, 0.7, 0.65),
                paint("Pants", 0.25, 0.2, 0.4),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_hr_bot",
        v("prim/hr_bot", 1.7, "HR Bot", "<Your Call Is Valued>", &[]),
    ),
    (
        "il_mirror_tank",
        v(
            BACKEND,
            1.9,
            "Copy (Tank)",
            "<A Perfect Copy. Lower Salary.>",
            &[glow("Skin", 0.35, 0.8, 1.0)],
        ),
    ),
    (
        "il_mirror_healer",
        v(
            DEVOPS,
            1.8,
            "Copy (Healer)",
            "<A Perfect Copy. Lower Salary.>",
            &[glow("Skin", 0.35, 0.8, 1.0)],
        ),
    ),
    (
        "il_mirror_dps",
        v(
            ENGINEER,
            1.8,
            "Copy (Damage)",
            "<A Perfect Copy. Lower Salary.>",
            &[glow("Skin", 0.35, 0.8, 1.0)],
        ),
    ),
    (
        "il_hrbp",
        v(
            SUIT_F,
            2.4,
            "HR Business Partner 2.0",
            "<Your Partner in Process>",
            &[
                paint("Black", 0.1, 0.16, 0.34),
                paint("Shirt", 0.95, 0.95, 0.98),
                glow("Hair", 0.3, 0.9, 1.0),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_cc_cpo",
        v(
            SUIT_M_BALD,
            2.3,
            "Chief People Officer",
            "<Our People Are Our Resources>",
            &[
                paint("Black", 0.28, 0.1, 0.22),
                paint("Shirt", 0.95, 0.95, 0.98),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_cc_culture",
        v(
            CASUAL2,
            2.2,
            "Head of Culture",
            "<Pizza Fridays>",
            &[
                paint("Shirt", 0.95, 0.5, 0.15),
                paint("Pants", 0.25, 0.25, 0.3),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_cc_vibes",
        v(
            CASUAL3,
            2.2,
            "Director of Vibes",
            "<Vibes. Directed.>",
            &[
                glow("Shirt", 0.9, 0.3, 1.0),
                paint("Pants", 0.2, 0.9, 0.8),
                paint("Skin", 0.74, 0.77, 0.8),
            ],
        ),
    ),
    (
        "il_stakeholder",
        v(
            "prim/stakeholder",
            2.2,
            "Stakeholder",
            "<Just A Quick Question>",
            &[],
        ),
    ),
    (
        "il_perf_review",
        v(
            "prim/perf_review",
            4.6,
            "Performance Review",
            "<Everyone Is A Three>",
            &[],
        ),
    ),
    // ---- VibeCon (raid): lanyard neon on black, the founders in vests ----
    (
        "vc_registration",
        v(
            CASUAL_F,
            1.7,
            "Registration Volunteer",
            "<VibeCon Crew>",
            &[
                paint("Shirt", 0.3, 1.0, 0.45),
                paint("Hair", 0.35, 0.22, 0.12),
                paint("Pants", 0.12, 0.12, 0.14),
            ],
        ),
    ),
    (
        "vc_prompt_engineer",
        v(
            CASUAL_M,
            1.8,
            "Prompt Engineer",
            "<Six-Figure Adjectives>",
            &[
                paint("Shirt", 0.55, 0.35, 0.9),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "vc_tenx_founder",
        v(
            SUIT_M,
            1.85,
            "'10x' Founder",
            "<Pre-Seed, Post-Truth>",
            &[
                paint("Vest", 0.35, 0.4, 0.48),
                paint("Shirt", 0.85, 0.9, 1.0),
                paint("Pants", 0.25, 0.32, 0.45),
            ],
        ),
    ),
    (
        "vc_booth_staff",
        v(
            CASUAL2,
            1.8,
            "Booth Staff",
            "<Agent Framework of the Week>",
            &[
                paint("Shirt", 1.0, 0.45, 0.1),
                paint("Pants", 0.08, 0.08, 0.1),
            ],
        ),
    ),
    (
        "vc_swag_goblin",
        v(
            CASUAL3,
            1.45,
            "Swag Goblin",
            "<Has Forty Tote Bags>",
            &[
                paint("Shirt", 1.0, 0.85, 0.15),
                paint("Pants", 0.2, 0.55, 0.3),
                paint("Skin", 0.62, 0.78, 0.45),
            ],
        ),
    ),
    (
        "vc_influencer",
        v(
            CASUAL_F,
            1.75,
            "LinkedIn Influencer",
            "<Filming You for Content>",
            &[
                paint("Shirt", 0.15, 0.45, 0.85),
                paint("Hair", 0.98, 0.85, 0.35),
                paint("Pants", 0.95, 0.95, 0.95),
            ],
        ),
    ),
    (
        "vc_hackathon_zombie",
        v(
            CASUAL2,
            1.8,
            "Hackathon Zombie",
            "<Hour 47, Still Pivoting>",
            &[
                paint("Shirt", 0.3, 0.3, 0.34),
                paint("Pants", 0.2, 0.2, 0.25),
                paint("Skin", 0.66, 0.74, 0.6),
            ],
        ),
    ),
    (
        "vc_web3_pivot",
        v(
            CASUAL_M,
            1.8,
            "Web3 Survivor",
            "<Formerly Blockchain, Now AI-Native>",
            &[
                paint("Shirt", 0.95, 0.6, 0.1),
                paint("Pants", 0.1, 0.1, 0.12),
                paint("Hair", 0.1, 0.1, 0.1),
            ],
        ),
    ),
    (
        "vc_vibe_coder",
        v(
            CASUAL2,
            2.3,
            "The Vibe Coder",
            "<Talk: I Haven't Read Code Since March>",
            &[
                paint("Shirt", 0.2, 1.0, 0.4),
                paint("Pants", 0.95, 0.25, 0.75),
            ],
        ),
    ),
    (
        "vc_unreviewed_pr",
        v(
            "prim/unreviewed_pr",
            1.4,
            "Unreviewed PR",
            "<+4,812 / -3>",
            &[],
        ),
    ),
    (
        "vc_prompt_whisperer",
        v(
            SUIT_F,
            2.3,
            "The Prompt Whisperer",
            "<Prompting Is the New Programming>",
            &[
                paint("Shirt", 0.7, 0.55, 1.0),
                paint("Pants", 0.18, 0.12, 0.3),
                paint("Hair", 0.92, 0.92, 0.95),
            ],
        ),
    ),
    (
        "vc_demo_god",
        v(
            CASUAL_M,
            2.3,
            "The Demo God",
            "<Head of DevRel>",
            &[
                paint("Shirt", 0.2, 0.85, 1.0),
                paint("Pants", 0.1, 0.1, 0.14),
                paint("Hair", 0.95, 0.75, 0.2),
            ],
        ),
    ),
    (
        "vc_demo_bug",
        v(
            "prim/demo_bug",
            0.9,
            "Live Demo Bug",
            "<Only Happens On Stage>",
            &[],
        ),
    ),
    (
        "vc_hype_moderator",
        v(
            SUIT_M,
            2.3,
            "The Hype Panel",
            "<Moderator>",
            &[
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Pants", 0.12, 0.14, 0.28),
            ],
        ),
    ),
    (
        "vc_hype_vc",
        v(
            SUIT_M,
            2.2,
            "Panelist: Seed-Stage VC",
            "<Has Never Shipped Anything>",
            &[
                paint("Vest", 0.4, 0.45, 0.5),
                paint("Shirt", 0.6, 0.78, 0.95),
                paint("Pants", 0.75, 0.68, 0.5),
            ],
        ),
    ),
    (
        "vc_hype_caio",
        v(
            SUIT_F,
            2.2,
            "Panelist: Chief AI Officer",
            "<In the Role Since Tuesday>",
            &[
                paint("Shirt", 0.95, 0.3, 0.75),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "vc_keynote_agi",
        v(
            SUIT_M_BALD,
            2.4,
            "The Keynote AGI-Announcer",
            "<Black Turtleneck>",
            &[
                paint("Shirt", 0.06, 0.06, 0.07),
                paint("Vest", 0.06, 0.06, 0.07),
                paint("Pants", 0.25, 0.32, 0.45),
            ],
        ),
    ),
    (
        "vc_agi_beta",
        v("prim/agi_beta", 2.0, "AGI (Beta)", "<Waitlist Only>", &[]),
    ),
    // The lab boss's summoned copies share its rig, a size smaller.
    (
        "agi_hallucination",
        v(
            "prim/agi_prototype",
            2.4,
            "AGI Prototype",
            "<Definitely Not Sentient>",
            &[],
        ),
    ),
    // ---- All-Hands Offsite (raid): resort staff in navy and gold, the company in fleece ----
    (
        "offsite_concierge",
        v(
            SUIT_F,
            1.72,
            "Offsite Concierge",
            "<Lakeside Resort & Conference Centre>",
            &[
                paint("Black", 0.12, 0.16, 0.34),
                paint("Details", 0.85, 0.68, 0.25),
                paint("Hair", 0.82, 0.66, 0.3),
            ],
        ),
    ),
    (
        "offsite_planner",
        v(
            CASUAL_F,
            1.72,
            "Offsite Event Planner",
            "<Agenda v14 (Final) (Final)>",
            &[
                paint("Shirt", 0.98, 0.55, 0.62),
                paint("Pants", 0.2, 0.22, 0.3),
                paint("Hair", 0.2, 0.12, 0.08),
            ],
        ),
    ),
    (
        "lanyard_enforcer",
        v(
            WORKER,
            1.85,
            "Lanyard Enforcer",
            "<People Ops>",
            &[
                paint("Vest", 0.15, 0.45, 0.95),
                paint("Shirt", 0.92, 0.92, 0.95),
                paint("Hat", 0.15, 0.45, 0.95),
            ],
        ),
    ),
    (
        "breakout_facilitator",
        v(
            CASUAL_M,
            1.8,
            "Breakout Room Facilitator",
            "<Owns the Sticky Notes>",
            &[
                paint("Shirt", 1.0, 0.9, 0.25),
                paint("Pants", 0.35, 0.3, 0.25),
            ],
        ),
    ),
    (
        "av_technician",
        v(
            WORKER,
            1.8,
            "AV Technician",
            "<Can You Hear Me Now>",
            &[
                paint("Vest", 0.1, 0.1, 0.12),
                paint("Shirt", 0.16, 0.16, 0.18),
                paint("Hat", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "open_bar_rep",
        v(
            SUIT_M,
            1.82,
            "Open Bar Account Executive",
            "<Expensing It>",
            &[
                paint("Black", 0.42, 0.08, 0.14),
                paint("Shirt", 0.95, 0.92, 0.85),
                paint("Details", 0.85, 0.68, 0.25),
            ],
        ),
    ),
    (
        "icebreaker_facilitator",
        v(
            CASUAL_F,
            2.3,
            "Icebreaker Facilitator",
            "<Certified Fun Practitioner>",
            &[
                paint("Shirt", 1.0, 0.35, 0.1),
                paint("Pants", 0.2, 0.75, 0.7),
                paint("Hair", 0.95, 0.4, 0.6),
            ],
        ),
    ),
    // The twins wear the same suit in opposite colours, so the open one is easy to call.
    (
        "synergy_alignment",
        v(
            SUIT_M,
            2.2,
            "Synergy Twin: Alignment",
            "<Co-Head of Synergy>",
            &[
                paint("Black", 0.12, 0.3, 0.8),
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Details", 0.95, 0.95, 0.95),
            ],
        ),
    ),
    (
        "synergy_leverage",
        v(
            SUIT_M,
            2.2,
            "Synergy Twin: Leverage",
            "<Other Co-Head of Synergy>",
            &[
                paint("Black", 0.9, 0.45, 0.08),
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Details", 0.95, 0.95, 0.95),
            ],
        ),
    ),
    (
        "keynote_speaker",
        v(
            SUIT_M,
            2.4,
            "Keynote Speaker",
            "<AI Hologram, Thought Leader>",
            &[
                glow("Black", 0.1, 0.5, 0.9),
                glow("Shirt", 0.25, 0.85, 1.0),
                glow("Details", 0.6, 0.95, 1.0),
                glow("Skin", 0.45, 0.9, 1.0),
                glow("Hair", 0.3, 0.7, 1.0),
            ],
        ),
    ),
    (
        "the_founder",
        v(
            SUIT_M_BALD,
            2.5,
            "The Founder",
            "<Chief Vision Officer, Majority Shareholder>",
            &[
                paint("Black", 0.38, 0.4, 0.43),
                paint("Shirt", 0.55, 0.7, 0.88),
                paint("Details", 0.55, 0.7, 0.88),
            ],
        ),
    ),
    (
        "founders_nephew",
        v(
            CASUAL2,
            1.75,
            "Founder's Nephew",
            "<Head of Growth, Age 23>",
            &[
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Pants", 0.3, 0.45, 0.7),
                paint("Hair", 0.75, 0.55, 0.25),
            ],
        ),
    ),
    (
        "cofounders_spouse",
        v(
            SUIT_F,
            1.75,
            "Co-Founder's Spouse",
            "<Chief People Officer>",
            &[
                paint("Black", 0.45, 0.1, 0.3),
                paint("Shirt", 0.95, 0.9, 0.82),
                paint("Hair", 0.9, 0.85, 0.75),
            ],
        ),
    ),
    // ---- Teambuilding minigame: the people you sneak past, colour-coded like their cones ----
    (
        "tb_manager",
        v(
            SUIT_M,
            1.82,
            "Engineering Manager",
            "<Let's Circle Back>",
            &[
                paint("Black", 0.22, 0.26, 0.34),
                paint("Shirt", 1.0, 0.72, 0.35),
                paint("Details", 0.95, 0.5, 0.1),
            ],
        ),
    ),
    (
        "tb_ai_talker",
        v(
            CASUAL_M,
            1.8,
            "AI Enthusiast",
            "<Let Me Tell You About AI>",
            &[
                glow("Shirt", 0.35, 0.65, 1.0),
                paint("Pants", 0.1, 0.1, 0.14),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "tb_hr_fun",
        v(
            CASUAL_F,
            1.72,
            "HR Fun Committee",
            "<Mandatory Fun Lead>",
            &[
                paint("Shirt", 1.0, 0.4, 0.75),
                paint("Pants", 0.98, 0.82, 0.25),
                paint("Hair", 0.95, 0.55, 0.2),
            ],
        ),
    ),
    (
        "tb_ceo",
        v(
            SUIT_M_BALD,
            2.0,
            "The CEO",
            "<Just Here for the Photo>",
            &[
                paint("Black", 0.08, 0.08, 0.1),
                paint("Shirt", 0.96, 0.96, 0.96),
                paint("Details", 0.85, 0.12, 0.12),
            ],
        ),
    ),
    // ---- Company Calendar minigames: the manager, the reorganised, the disturbers ----
    (
        "pr_manager",
        v(
            SUIT_F,
            1.74,
            "Your Manager",
            "<Has a Hard Stop>",
            &[
                paint("Black", 0.24, 0.26, 0.32),
                paint("Shirt", 0.92, 0.9, 0.86),
                paint("Hair", 0.3, 0.2, 0.14),
            ],
        ),
    ),
    (
        "rg_colleague_1",
        v(
            CASUAL_M,
            1.78,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Shirt", 0.35, 0.55, 0.85),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "rg_colleague_2",
        v(
            CASUAL_F,
            1.7,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Shirt", 0.85, 0.45, 0.35),
                paint("Hair", 0.2, 0.12, 0.08),
            ],
        ),
    ),
    (
        "rg_colleague_3",
        v(
            CASUAL2,
            1.8,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Shirt", 0.4, 0.7, 0.45),
                paint("Pants", 0.3, 0.26, 0.2),
            ],
        ),
    ),
    (
        "rg_colleague_4",
        v(
            SUIT_M,
            1.82,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Black", 0.3, 0.3, 0.36),
                paint("Shirt", 0.95, 0.85, 0.5),
            ],
        ),
    ),
    (
        "rg_colleague_5",
        v(
            CASUAL3,
            1.76,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Shirt", 0.7, 0.45, 0.8),
                paint("Pants", 0.25, 0.25, 0.28),
            ],
        ),
    ),
    (
        "rg_colleague_6",
        v(
            SUIT_F,
            1.72,
            "Confused Colleague",
            "<Reorganised>",
            &[
                paint("Black", 0.4, 0.3, 0.2),
                paint("Shirt", 0.8, 0.9, 0.95),
            ],
        ),
    ),
    (
        "tm_champion",
        v(
            ENGINEER,
            1.8,
            "Migration Champion",
            "<FlowSphere Evangelist>",
            &[
                glow("Shirt", 0.3, 0.75, 0.95),
                paint("Pants", 0.15, 0.15, 0.2),
            ],
        ),
    ),
    (
        "hk_mc",
        v(
            CASUAL_M,
            1.84,
            "Hackathon MC",
            "<Disrupting Saturday>",
            &[
                glow("Shirt", 1.0, 0.45, 0.2),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "hk_hacker",
        v(
            CASUAL2,
            1.76,
            "Hackathon Teammate",
            "<Pivoting>",
            &[
                paint("Shirt", 0.2, 0.2, 0.24),
                paint("Pants", 0.25, 0.3, 0.45),
            ],
        ),
    ),
    (
        "os_colleague",
        v(
            CASUAL_M,
            1.78,
            "Got-a-Minute Colleague",
            "<Quick Question>",
            &[
                paint("Shirt", 1.0, 0.62, 0.2),
                paint("Pants", 0.25, 0.25, 0.3),
            ],
        ),
    ),
    (
        "os_caller",
        v(
            SUIT_M,
            1.84,
            "Speakerphone Account Manager",
            "<On a Call, Loudly>",
            &[
                paint("Black", 0.12, 0.2, 0.4),
                glow("Shirt", 0.35, 0.65, 1.0),
            ],
        ),
    ),
    (
        "os_cake",
        v(
            CASUAL_F,
            1.7,
            "Birthday Committee",
            "<It's Someone's Birthday>",
            &[
                paint("Shirt", 1.0, 0.4, 0.75),
                paint("Hair", 0.95, 0.75, 0.3),
            ],
        ),
    ),
    (
        "os_warden",
        v(
            WORKER,
            1.82,
            "Fire Warden",
            "<Drill Test, Please Ignore>",
            &[
                glow("Shirt", 1.0, 0.25, 0.15),
                paint("Pants", 0.2, 0.2, 0.2),
            ],
        ),
    ),
    // ---- variety pass ----
    (
        "av_floor_monitor",
        v(
            "prim/cooling_fan",
            1.2,
            "Floor Monitor Speaker",
            "<Pointed at the Wrong Aisle>",
            &[],
        ),
    ),
    (
        "vc_tote_bag",
        v(
            "prim/delivery_robot",
            0.9,
            "Sentient Tote Bag",
            "<Contains Nine Lanyards>",
            &[],
        ),
    ),
    (
        "oom_killer",
        v(
            WORKER,
            1.95,
            "OOM Killer",
            "<Picks the Biggest Process>",
            &[
                glow("Shirt", 0.9, 0.15, 0.1),
                paint("Pants", 0.1, 0.1, 0.12),
            ],
        ),
    ),
    (
        "noc_operator",
        v(
            CASUAL_F,
            1.75,
            "NOC Operator",
            "<Watching 400 Graphs, Trusting None>",
            &[
                paint("Shirt", 0.15, 0.35, 0.3),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "backup_job",
        v(
            "prim/overheated_rack",
            2.2,
            "The Nightly Backup Job",
            "<Last Success: Never>",
            &[],
        ),
    ),
    (
        "closed_model",
        v(
            SUIT_M,
            2.0,
            "Closed-Source Frontier Model",
            "<Weights Not Included>",
            &[
                glow("Shirt", 0.2, 0.2, 0.25),
                paint("Pants", 0.05, 0.05, 0.06),
            ],
        ),
    ),
    (
        "red_team_lead",
        v(
            CASUAL_M,
            1.8,
            "AI Red Team Lead",
            "<Paid to Break Things>",
            &[
                paint("Shirt", 0.75, 0.1, 0.12),
                paint("Pants", 0.15, 0.15, 0.18),
            ],
        ),
    ),
    // ---- Peg Hollis, the reviewer at every campaign zone's far end: one look, a title per floor.
    (
        "reviewer_t1",
        peg("<Principal Engineer, Rehired as Contractor>"),
    ),
    ("reviewer_t2", peg("<Reviews Everything, Twice>")),
    (
        "reviewer_t3",
        peg("<Has Opinions About Your Commit Messages>"),
    ),
    (
        "reviewer_dc1",
        peg("<Brought a Sweater and a 2011 Network Diagram>"),
    ),
    ("reviewer_dc2", peg("<Keeper of the Last Tape>")),
    ("reviewer_t4", peg("<Sticker Budget: Unlimited>")),
    ("reviewer_t5", peg("<Has Seen the CEO's Prompt>")),
    ("reviewer_lab1", peg("<Not Technically Anywhere>")),
    ("reviewer_lab2", peg("<Final Approver>")),
    (
        "cto",
        v(
            SUIT_M_BALD,
            1.82,
            "The CTO",
            "<Technically the Officer>",
            &[
                paint("Shirt", 0.2, 0.22, 0.3),
                paint("Pants", 0.12, 0.12, 0.14),
            ],
        ),
    ),
    (
        "pip_colleague",
        v(
            CASUAL_M,
            1.76,
            "Dave from QA",
            "<On a Performance Improvement Plan>",
            &[
                paint("Shirt", 0.55, 0.6, 0.45),
                paint("Pants", 0.3, 0.28, 0.25),
            ],
        ),
    ),
    (
        "tape_robot",
        v(
            "prim/overheated_rack",
            1.2,
            "TAPE-9000",
            "<Carrying the Last Real Backup>",
            &[],
        ),
    ),
    (
        "rogue_calendar_invite",
        v(
            "prim/demo_bug",
            1.0,
            "Recurring Calendar Invite",
            "<Accepted on Your Behalf>",
            &[],
        ),
    ),
    (
        "zombie_process",
        v(
            "prim/agi_beta",
            1.3,
            "Zombie Process",
            "<Parent: None>",
            &[],
        ),
    ),
    (
        "jira_automation",
        v(
            "prim/agi_beta",
            1.7,
            "Jira Automation Rule #47",
            "<Last Edited By: Nobody>",
            &[],
        ),
    ),
    (
        "hard_deadline",
        v(
            SUIT_M_BALD,
            2.2,
            "The Hard Deadline",
            "<Not Moving This Time>",
            &[
                glow("Shirt", 0.8, 0.1, 0.1),
                paint("Pants", 0.05, 0.05, 0.05),
            ],
        ),
    ),
    (
        "oncall_engineer",
        v(
            CASUAL2,
            1.8,
            "On-Call Engineer",
            "<Pager Duty, Week 9 of 1>",
            &[
                paint("Shirt", 0.35, 0.35, 0.4),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "prod_server",
        v(
            "prim/overheated_rack",
            1.6,
            "Prod (Do Not Touch)",
            "<The Only Server That Matters>",
            &[],
        ),
    ),
    (
        "pager_alert",
        v(
            "prim/demo_bug",
            1.0,
            "Unhandled Pager Alert",
            "<Severity: Yes>",
            &[],
        ),
    ),
    (
        "qa_lead",
        v(
            CASUAL_F,
            1.7,
            "QA Lead",
            "<Tests in Production, Officially Now>",
            &[
                paint("Shirt", 0.3, 0.75, 0.35),
                paint("Hair", 0.6, 0.3, 0.15),
            ],
        ),
    ),
    // ---- The Onboarding (dungeon): pastel office staff, bots and the chatty buddy ----
    (
        "welcome_desk_clerk",
        v(
            CASUAL_F,
            1.7,
            "Welcome Desk Clerk",
            "<Your Badge Is Being Printed>",
            &[
                paint("Shirt", 0.55, 0.85, 0.78),
                paint("Pants", 0.3, 0.35, 0.45),
                paint("Hair", 0.35, 0.22, 0.12),
            ],
        ),
    ),
    (
        "tired_new_hire",
        v(
            CASUAL_M,
            1.75,
            "Very Tired New Hire",
            "<Day 5>",
            &[
                paint("Shirt", 0.55, 0.57, 0.62),
                paint("Pants", 0.25, 0.27, 0.32),
            ],
        ),
    ),
    (
        "badge_scanner",
        v(
            "prim/delivery_robot",
            1.5,
            "Badge Scanner",
            "<Beeps Wrong Every Time>",
            &[],
        ),
    ),
    (
        "welcome_packet_dispenser",
        v(
            "prim/espresso_cart",
            1.4,
            "Welcome Packet Dispenser",
            "<Dispatches Swag>",
            &[],
        ),
    ),
    (
        "hr_generalist",
        v(
            CASUAL_F,
            1.7,
            "HR Generalist",
            "<Circles Back>",
            &[
                paint("Shirt", 0.95, 0.6, 0.55),
                paint("Pants", 0.25, 0.25, 0.35),
                paint("Hair", 0.6, 0.35, 0.15),
            ],
        ),
    ),
    (
        "probation_intern",
        v(
            CASUAL_M,
            1.55,
            "Forgotten Probation Intern",
            "<Day 89 of 90>",
            &[
                paint("Shirt", 0.7, 0.85, 0.95),
                paint("Pants", 0.35, 0.35, 0.4),
            ],
        ),
    ),
    (
        "org_chart",
        v(
            "prim/cooling_fan",
            1.7,
            "Org Chart",
            "<Forever Updating>",
            &[],
        ),
    ),
    (
        "name_tag",
        v("prim/drone", 0.55, "Name Tag", "<Hello, My Name Is>", &[]),
    ),
    (
        "attendance_sheet",
        v(
            "prim/drone",
            0.8,
            "Attendance Sheet",
            "<Please Sign Here>",
            &[],
        ),
    ),
    (
        "courier_drone",
        v(
            "prim/drone",
            1.2,
            "Courier Drone",
            "<Escalates to Tier 2>",
            &[],
        ),
    ),
    (
        "compliance_trainer",
        v(
            SUIT_M,
            1.95,
            "Compliance Trainer",
            "<Certified, Annually>",
            &[
                paint("Black", 0.18, 0.28, 0.4),
                paint("Shirt", 0.95, 0.95, 0.98),
                paint("Skin", 0.95, 0.8, 0.7),
            ],
        ),
    ),
    (
        "laptop_provisioner",
        v(
            WORKER,
            2.1,
            "Laptop Provisioner",
            "<INC-0042 (Awaiting Hardware)>",
            &[
                paint("Vest", 0.98, 0.55, 0.12),
                paint("Shirt", 0.9, 0.9, 0.92),
                paint("Hat", 0.95, 0.95, 0.95),
            ],
        ),
    ),
    (
        "onboarding_buddy",
        v(
            CASUAL_M,
            2.0,
            "Onboarding Buddy",
            "<Your Point of Contact>",
            &[
                paint("Shirt", 0.4, 0.8, 0.95),
                paint("Pants", 0.85, 0.85, 0.9),
            ],
        ),
    ),
    // ---- The Migration (dungeon): basement staff, legacy processes, the consultants and the cutover ----
    (
        "migration_pm",
        v(
            CASUAL_F,
            1.7,
            "Migration Project Manager",
            "<Cutover Is Friday>",
            &[
                paint("Shirt", 0.55, 0.78, 0.4),
                paint("Pants", 0.22, 0.25, 0.32),
                paint("Hair", 0.15, 0.1, 0.08),
            ],
        ),
    ),
    (
        "migration_oncall",
        v(
            CASUAL_M,
            1.75,
            "On-Call Engineer",
            "<Weekend 14>",
            &[
                paint("Shirt", 0.3, 0.32, 0.38),
                paint("Pants", 0.18, 0.2, 0.25),
                paint("Hair", 0.25, 0.2, 0.15),
            ],
        ),
    ),
    (
        "orphaned_process",
        v(
            "prim/drone",
            1.1,
            "Orphaned Process",
            "<Still Running, Nobody Knows Why>",
            &[],
        ),
    ),
    (
        "defunct_process",
        v(
            "prim/demo_bug",
            0.9,
            "Zombie",
            "<Defunct, Technically>",
            &[],
        ),
    ),
    (
        "cable_spaghetti",
        v(
            "prim/demo_bug",
            1.5,
            "Cable Spaghetti",
            "<Do Not Touch>",
            &[],
        ),
    ),
    (
        "legacy_admin",
        v(COBOL, 1.8, "Legacy Admin", "<Last Person Who Knew>", &[]),
    ),
    (
        "migration_script",
        v(
            "prim/drone",
            1.0,
            "Migration Script",
            "<v0.1, Untested>",
            &[],
        ),
    ),
    (
        "backup_tape_robot",
        v(
            "prim/overheated_rack",
            1.5,
            "Tape Robot",
            "<Backup, Technically>",
            &[],
        ),
    ),
    (
        "stale_process",
        v("prim/drone", 0.9, "Stale Process", "<Holding a Lock>", &[]),
    ),
    (
        "junior_analyst",
        v(
            CASUAL_M,
            1.6,
            "Junior Analyst",
            "<Slides, by EOD>",
            &[
                paint("Shirt", 0.85, 0.88, 0.95),
                paint("Pants", 0.2, 0.22, 0.3),
            ],
        ),
    ),
    (
        "orphaned_record",
        v(
            "prim/drone",
            0.8,
            "Orphaned Record",
            "<Foreign Key: Missing>",
            &[],
        ),
    ),
    (
        "rollback_wraith",
        v(
            SUIT_M_BALD,
            2.4,
            "Rollback Wraith",
            "<Ghost of the Previous Release>",
            &[
                paint("Black", 0.55, 0.7, 0.85),
                paint("Shirt", 0.8, 0.9, 1.0),
                glow("Skin", 0.35, 0.7, 1.0),
            ],
        ),
    ),
    (
        "consultant_jolene",
        v(
            SUIT_F,
            2.0,
            "Jolene Utilization",
            "<Senior Manager, Billable>",
            &[
                paint("Black", 0.1, 0.1, 0.14),
                paint("Shirt", 0.9, 0.3, 0.45),
                paint("Hair", 0.55, 0.25, 0.1),
            ],
        ),
    ),
    (
        "consultant_jack",
        v(
            SUIT_M,
            2.1,
            "Jack Rate-Card",
            "<Engagement Partner>",
            &[
                paint("Black", 0.12, 0.14, 0.2),
                paint("Shirt", 0.9, 0.9, 0.95),
                paint("Skin", 0.9, 0.72, 0.6),
            ],
        ),
    ),
    (
        "big_bang_cutover",
        v(
            "prim/cutover_rack",
            3.8,
            "Big Bang Cutover",
            "<Go-Live Weekend>",
            &[],
        ),
    ),
    // ---- Quarterly Business Review (dungeon): boardroom staff, stakeholders, the deck, the analyst and the Board ----
    (
        "exec_assistant",
        v(
            CASUAL_F,
            1.7,
            "Executive Assistant",
            "<Owns Everyone's Calendar>",
            &[
                paint("Shirt", 0.9, 0.85, 0.75),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Hair", 0.75, 0.6, 0.3),
            ],
        ),
    ),
    (
        "boardroom_cleaner",
        v(
            WORKER,
            1.75,
            "Cleaner of the Boardroom",
            "<Has Heard Everything>",
            &[paint("Shirt", 0.25, 0.45, 0.75)],
        ),
    ),
    (
        "stakeholder",
        v(
            SUIT_M,
            1.85,
            "Stakeholder",
            "<Needs Visibility>",
            &[
                paint("Black", 0.2, 0.22, 0.3),
                paint("Shirt", 0.95, 0.95, 0.95),
                paint("Skin", 0.85, 0.65, 0.5),
            ],
        ),
    ),
    (
        "strategy_intern",
        v(
            CASUAL_M,
            1.6,
            "Strategy Intern",
            "<Made The Deck>",
            &[
                paint("Shirt", 0.3, 0.6, 0.85),
                paint("Pants", 0.2, 0.2, 0.25),
                paint("Hair", 0.4, 0.25, 0.1),
            ],
        ),
    ),
    (
        "synergy_coordinator",
        v(
            SUIT_F,
            1.75,
            "Synergy Coordinator",
            "<Aligned On Alignment>",
            &[
                paint("Black", 0.1, 0.5, 0.5),
                paint("Shirt", 0.95, 0.9, 0.7),
                paint("Hair", 0.15, 0.1, 0.1),
            ],
        ),
    ),
    (
        "kpi_dashboard",
        v(
            "prim/kpi_dashboard",
            1.7,
            "KPI Dashboard",
            "<Always Green>",
            &[],
        ),
    ),
    (
        "catering_drone",
        v("prim/drone", 1.0, "Catering Drone", "<Free Pastry>", &[]),
    ),
    (
        "stale_pastry",
        v(
            "prim/espresso_cart",
            0.7,
            "Stale Croissant",
            "<Untouched Since 07:00>",
            &[],
        ),
    ),
    (
        "bullet_point",
        v(
            "prim/drone",
            0.8,
            "Bullet Point",
            "<Sub-Bullet Of Slide 6>",
            &[],
        ),
    ),
    (
        "generated_chart",
        v(
            "prim/delivery_robot",
            1.2,
            "Generated Chart",
            "<Axes: Swapped>",
            &[],
        ),
    ),
    (
        "slide_deck_golem",
        v(
            "prim/slide_golem",
            4.2,
            "Slide Deck Golem",
            "<Deck_v37_FINAL_final2.pptx>",
            &[],
        ),
    ),
    (
        "analyst_dana",
        v(
            SUIT_F,
            2.1,
            "Dana",
            "<Who Had Claude Do The Analysis>",
            &[
                paint("Black", 0.35, 0.2, 0.55),
                paint("Shirt", 0.55, 1.0, 0.8),
                glow("Hair", 0.4, 0.9, 0.7),
            ],
        ),
    ),
    (
        "board_chair",
        v(
            SUIT_M_BALD,
            2.5,
            "The Chair",
            "<A Quorum of Three>",
            &[
                paint("Black", 0.08, 0.08, 0.1),
                paint("Shirt", 0.95, 0.9, 0.7),
                paint("Skin", 0.85, 0.62, 0.5),
            ],
        ),
    ),
    (
        "board_activist",
        v(
            SUIT_M,
            2.2,
            "The Activist Investor",
            "<Here To Improve The Multiple>",
            &[
                paint("Black", 0.5, 0.05, 0.08),
                paint("Shirt", 0.15, 0.15, 0.2),
                paint("Skin", 0.75, 0.55, 0.45),
            ],
        ),
    ),
    (
        "board_independent",
        v(
            SUIT_F,
            2.2,
            "The Independent Director",
            "<Same Golf Partner As The Chair>",
            &[
                paint("Black", 0.85, 0.85, 0.9),
                paint("Shirt", 0.35, 0.55, 0.9),
                paint("Hair", 0.85, 0.85, 0.85),
            ],
        ),
    ),
    // ---- The Hackathon (heroic dungeon): organisers, trash, adds, the Pizza Overlord, the Gremlin and the Judge ----
    (
        "hackathon_organiser",
        v(
            CASUAL_F,
            1.7,
            "Hackathon Organiser",
            "<Free Pizza, Unlimited>",
            &[
                paint("Shirt", 0.95, 0.3, 0.8),
                paint("Pants", 0.12, 0.12, 0.2),
                paint("Hair", 0.1, 0.8, 0.9),
            ],
        ),
    ),
    (
        "clipboard_volunteer",
        v(
            WORKER,
            1.75,
            "Volunteer With Clipboard",
            "<Has Been Here Since Friday>",
            &[paint("Shirt", 0.95, 0.55, 0.15)],
        ),
    ),
    (
        "sleepy_dev",
        v(
            CASUAL_M,
            1.7,
            "Sleep-Deprived Developer",
            "<Day 2>",
            &[
                paint("Shirt", 0.25, 0.2, 0.4),
                paint("Pants", 0.2, 0.2, 0.25),
                paint("Skin", 0.78, 0.68, 0.62),
            ],
        ),
    ),
    (
        "prompt_engineer",
        v(
            CASUAL_F,
            1.7,
            "Prompt Engineer",
            "<Types Words At A Chatbot>",
            &[
                paint("Shirt", 0.1, 0.55, 0.55),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Hair", 0.6, 0.15, 0.7),
            ],
        ),
    ),
    (
        "hallucination",
        v(
            "prim/hallucination",
            1.2,
            "Hallucination",
            "<Sounds Right>",
            &[],
        ),
    ),
    (
        "energy_pyramid",
        v(
            "prim/energy_pyramid",
            1.6,
            "Energy Drink Pyramid",
            "<47 Cans>",
            &[],
        ),
    ),
    (
        "lead_mentor",
        v(
            SUIT_M,
            1.9,
            "Lead Mentor",
            "<Giving Feedback>",
            &[
                paint("Black", 0.3, 0.3, 0.35),
                paint("Shirt", 0.95, 0.85, 0.2),
                paint("Skin", 0.7, 0.5, 0.4),
            ],
        ),
    ),
    (
        "team_ai",
        v(
            CASUAL_M,
            1.55,
            "Team AI",
            "<All Seventeen Of Them>",
            &[
                paint("Shirt", 0.2, 0.9, 1.0),
                paint("Pants", 0.15, 0.15, 0.2),
                paint("Hair", 0.1, 0.1, 0.1),
            ],
        ),
    ),
    (
        "pizza_slice",
        v("prim/pizza_slice", 0.9, "Slice", "<Cold Since 3 AM>", &[]),
    ),
    (
        "heckler",
        v(
            CASUAL_F,
            1.6,
            "Heckler",
            "<Back Row, Third Seat>",
            &[paint("Shirt", 0.8, 0.2, 0.2), paint("Hair", 0.3, 0.2, 0.1)],
        ),
    ),
    (
        "associate",
        v(
            SUIT_F,
            1.75,
            "Associate",
            "<Fleece Vest, Eager>",
            &[
                paint("Black", 0.2, 0.3, 0.5),
                paint("Shirt", 0.9, 0.9, 0.95),
                paint("Hair", 0.6, 0.4, 0.2),
            ],
        ),
    ),
    (
        "pizza_overlord",
        v(
            "prim/pizza_overlord",
            4.4,
            "Pizza Overlord",
            "<Free Pizza Is Never Free>",
            &[],
        ),
    ),
    (
        "demo_gremlin",
        v(
            "prim/demo_gremlin",
            2.6,
            "Demo Day Gremlin",
            "<It Worked On My Machine>",
            &[],
        ),
    ),
    (
        "pitch_judge",
        v(
            SUIT_M,
            2.5,
            "Pitch Judge VC",
            "<Only Funds AI>",
            &[
                paint("Black", 0.12, 0.2, 0.42),
                paint("Shirt", 0.95, 0.95, 0.98),
                paint("Skin", 0.82, 0.6, 0.48),
                glow("Hair", 1.0, 0.85, 0.3),
            ],
        ),
    ),
];

/// Primitive rig kind (see `rigs.rs`) of a unit key, `None` for character-model units.
pub fn primitive_rig(key: &str) -> Option<&'static str> {
    is_known(key)
        .then(|| unit_visual(key).model.strip_prefix(PRIM))
        .flatten()
}

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

/// Who stands in a pack of a campus, data center or lab zone (debug dummies and the content
/// reference); `None` for zones the towers' `pack_roster` covers.
pub fn zone_pack_roster(zone: &str, pack: usize) -> Option<&'static [&'static str]> {
    const CAMPUS: [&[&str]; 8] = [
        &["delivery_robot", "delivery_robot"],
        &["scooter_exec", "unpaid_ambassador"],
        &["delivery_robot", "unpaid_ambassador", "scooter_exec"],
        &["scooter_exec", "scooter_exec"],
        &["unpaid_ambassador", "delivery_robot", "delivery_robot"],
        &["scooter_exec", "unpaid_ambassador", "unpaid_ambassador"],
        &["delivery_robot", "delivery_robot", "scooter_exec"],
        &["unpaid_ambassador", "unpaid_ambassador"],
    ];
    const DC1: [&[&str]; 5] = [
        &["gpu_cultist", "gpu_cultist", "cooling_fan"],
        &["token_counter", "gpu_cultist"],
        &["cooling_fan", "cooling_fan", "overheated_rack"],
        &["rogue_agent", "gpu_cultist", "token_counter"],
        &[
            "gpu_cultist",
            "gpu_cultist",
            "gpu_cultist",
            "overheated_rack",
        ],
    ];
    const DC2: [&[&str]; 5] = [
        &["overheated_rack", "overheated_rack", "cooling_fan"],
        &["rogue_agent", "token_counter"],
        &["cooling_fan", "overheated_rack", "gpu_cultist"],
        &["overheated_rack", "overheated_rack", "rogue_agent"],
        &[
            "cooling_fan",
            "cooling_fan",
            "overheated_rack",
            "gpu_cultist",
        ],
    ];
    const LAB1: [&[&str]; 5] = [
        &["demo_presenter", "prompt_whisperer"],
        &["vc_partner", "demo_presenter"],
        &["prompt_whisperer", "prompt_whisperer", "demo_presenter"],
        &["vc_partner", "vc_partner"],
        &["demo_presenter", "prompt_whisperer", "vc_partner"],
    ];
    const LAB2: [&[&str]; 5] = [
        &["rogue_agent", "prompt_whisperer"],
        &["chatbot_minion", "chatbot_minion", "rogue_agent"],
        &["token_counter", "prompt_whisperer", "chatbot_minion"],
        &["rogue_agent", "rogue_agent"],
        &[
            "prompt_whisperer",
            "prompt_whisperer",
            "token_counter",
            "chatbot_minion",
        ],
    ];
    let table: &[&[&str]] = match zone {
        "campus" => &CAMPUS,
        "datacenter/1" => &DC1,
        "datacenter/2" => &DC2,
        "lab/1" => &LAB1,
        "lab/2" => &LAB2,
        _ => return None,
    };
    table.get(pack).copied()
}

/// Boss unit key of a non-tower zone.
pub fn zone_boss_key(zone: &str) -> Option<&'static str> {
    match zone {
        "datacenter/1" => Some("the_scheduler"),
        "datacenter/2" => Some("cooling_failure"),
        "lab/1" => Some("vc_partner"),
        "lab/2" => Some("agi_prototype"),
        _ => None,
    }
}

/// Friendly NPC at a named anchor of a non-tower zone. The campus order is: intern, the three
/// quest givers (facilities guy, barista, smoking-corner dev), then two ambient NPCs.
pub fn zone_npc_at(zone: &str, anchor: &str) -> Option<(&'static str, IdlePose)> {
    match (zone, anchor) {
        ("campus", "npc_1") => Some(("intern", IdlePose::Standing)),
        ("campus", "npc_2") => Some(("facilities_guy", IdlePose::Standing)),
        ("campus", "npc_3") => Some(("barista", IdlePose::Standing)),
        ("campus", "npc_4") => Some(("smoking_corner_dev", IdlePose::Sitting)),
        ("campus", "npc_5") => Some(("unpaid_ambassador", IdlePose::Chatting)),
        ("campus", "npc_6") => Some(("scooter_exec", IdlePose::Standing)),
        ("datacenter/1", "npc_1") => Some(("it_support", IdlePose::Standing)),
        ("datacenter/2", "npc_1") => Some(("it_support", IdlePose::Sleeping)),
        ("lab/1", "npc_1") => Some(("intern", IdlePose::Chatting)),
        ("lab/2", "npc_1") => Some(("ai_ethicist", IdlePose::Standing)),
        _ => None,
    }
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
        "frontend_dev",
        "backend_dev",
        "devops_eng",
        "qa_engineer",
        "data_scientist",
        "security_engineer",
        "cobol_greybeard",
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
        "facilities_guy",
        "procurement_officer",
        "hardware_broker",
        "barista",
        "helpdesk_lead",
        "facilities_manager",
        "office_manager",
        "pmo_intern",
        "executive_assistant",
        "night_shift_sysadmin",
        "lab_safety_officer",
        "smoking_corner_dev",
        "delivery_robot",
        "scooter_exec",
        "unpaid_ambassador",
        "gpu_cultist",
        "cooling_fan",
        "token_counter",
        "rogue_agent",
        "the_scheduler",
        "cooling_failure",
        "overheated_rack",
        "demo_presenter",
        "vc_partner",
        "prompt_whisperer",
        "agi_prototype",
        "defensive_stakeholder",
        "agent_orchestrator",
        "strategy_consultant",
        "chief_happiness_officer",
        "chief_of_staff",
        "shadow_it_server",
        "hype_cycle_analyst",
    ];

    /// A unit without an entry renders as the "Unknown" fallback, so every content unit needs one.
    #[test]
    fn every_content_unit_is_mapped() {
        let sim = crate::combat::new_sim().unwrap_or_else(|e| panic!("{e:?}"));
        let missing: Vec<&str> = sim
            .content()
            .units
            .iter()
            .map(|u| u.key.as_str())
            .filter(|k| !is_known(k))
            .collect();
        assert!(missing.is_empty(), "unmapped units: {missing:?}");
    }

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
            if primitive_rig(k).is_some() {
                continue;
            }
            assert!(dir.join(vis.model).exists(), "{k}: {}", vis.model);
            assert!(CHARACTER_MODELS.contains(&vis.model), "{k}: {}", vis.model);
            assert!(!vis.title.is_empty());
        }
        // Every GLB is about 1.3 MB of download; one per playable class plus ten NPC bodies.
        assert!(CHARACTER_MODELS.len() <= 18);
    }

    #[test]
    fn zone_rosters_only_name_known_units() {
        for zone in ["campus", "datacenter/1", "datacenter/2", "lab/1", "lab/2"] {
            for pack in 0..8 {
                for k in zone_pack_roster(zone, pack).unwrap_or(&[]) {
                    assert!(is_known(k), "{zone} pack {pack}: {k}");
                }
            }
            for n in 1..=6 {
                if let Some((k, _)) = zone_npc_at(zone, &format!("npc_{n}")) {
                    assert!(is_known(k), "{zone} npc_{n}: {k}");
                }
            }
            if let Some(b) = zone_boss_key(zone) {
                assert!(is_known(b));
            }
        }
        assert_eq!(primitive_rig("agi_prototype"), Some("agi_prototype"));
        assert_eq!(primitive_rig("ceo"), None);
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
