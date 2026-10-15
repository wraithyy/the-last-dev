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
const CASUAL_F: &str = "models/characters/Casual_Female.glb";
const CASUAL2: &str = "models/characters/Casual2_Male.glb";
const CASUAL_M: &str = "models/characters/Casual_Male.glb";
const DOC_M: &str = "models/characters/Doctor_Male_Young.glb";
const DOC_F: &str = "models/characters/Doctor_Female_Old.glb";
const WORKER: &str = "models/characters/Worker_Male.glb";
/// Model prefix of units built from primitives (`rigs.rs`) instead of a character GLB.
const PRIM: &str = "prim/";

/// Every character GLB the catalog references.
pub const CHARACTER_MODELS: [&str; 14] = [
    ENGINEER,
    FRONTEND,
    BACKEND,
    DEVOPS,
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
        assert!(CHARACTER_MODELS.len() <= 14);
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
