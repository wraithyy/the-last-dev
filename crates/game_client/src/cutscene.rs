//! Short in-engine cutscenes after the campaign bosses (letterbox bars, a slow camera move to the
//! fallen boss, two to four subtitled lines) and the epilogue after the AGI reveal, which ends in
//! the credits roll. Each raid has its own: an intro on the first walk into its first room, one
//! per boss, and the closing remarks right after the final boss's. They are presentation only:
//! the sim keeps running (online the server never waits), Esc, a tap or the skip button ends
//! one, and sprint runs never play them.
//!
//! QA hook: `WOW_CUTSCENE=<boss key>@<secs>` plays that boss's scene at that time (at the player);
//! `WOW_CREDITS=<secs>` starts the credits roll.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::{CameraCeiling, Colliders};
use crate::camera::OrbitCamera;
use crate::collision::to_core;
use crate::combat::{CombatEvent, SimReady, SimView};
use crate::hud::{ACCENT, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::player::Player;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use combat_core::{Event, line_of_sight};

pub struct Line {
    pub who: &'static str,
    pub text: &'static str,
}

pub struct Scene {
    /// Unit key of the boss whose death plays it.
    pub boss: &'static str,
    pub title: &'static str,
    pub lines: &'static [Line],
}

const fn l(who: &'static str, text: &'static str) -> Line {
    Line { who, text }
}

pub const EPILOGUE: &str = "agi_prototype";
/// A raid's scenes that no death starts: the intro (its first zone entered) and the closing
/// remarks (straight after the final boss's scene).
pub struct RaidScenes {
    pub first_zone: &'static str,
    pub intro: &'static str,
    pub final_boss: &'static str,
    pub end: &'static str,
}

pub const RAID_SCENES: [RaidScenes; 8] = [
    RaidScenes {
        first_zone: "offsite/1",
        intro: "offsite_intro",
        final_boss: "the_founder",
        end: "offsite_end",
    },
    RaidScenes {
        first_zone: "vibecon/1",
        intro: "vibecon_intro",
        final_boss: "vc_keynote_agi",
        end: "vibecon_end",
    },
    RaidScenes {
        first_zone: "ilmari1/1",
        intro: "il_wing1_intro",
        final_boss: "il_cable_hydra",
        end: "il_wing1_end",
    },
    RaidScenes {
        first_zone: "ilmari2/1",
        intro: "il_wing2_intro",
        final_boss: "il_perf_review",
        end: "il_wing2_end",
    },
    RaidScenes {
        first_zone: "ilmari3/1",
        intro: "il_wing3_intro",
        final_boss: "il_model_collapse",
        end: "il_wing3_end",
    },
    RaidScenes {
        first_zone: "ilmari4/1",
        intro: "il_wing4_intro",
        final_boss: "il_region_outage",
        end: "il_wing4_end",
    },
    RaidScenes {
        first_zone: "ilmari5/1",
        intro: "il_wing5_intro",
        final_boss: "il_red_team",
        end: "il_wing5_end",
    },
    RaidScenes {
        first_zone: "ilmari6/1",
        intro: "il_wing6_intro",
        final_boss: "il_cio",
        end: "il_wing6_end",
    },
];

/// Scene keys that are not units (raid intros and closing remarks).
#[cfg(test)]
fn bookend(key: &str) -> bool {
    RAID_SCENES.iter().any(|r| r.intro == key || r.end == key)
}

/// Story milestones of "The Unsigned Commit": these scenes play on the local player's
/// turn-in of the quest with this id (filmed at the player), not on a death.
pub const QUEST_SCENES: [&str; 3] = [
    "story_who_pushed",
    "story_last_backup",
    "story_unplug_the_assistant",
];

pub const SCENES: &[Scene] = &[
    Scene {
        boss: "story_who_pushed",
        title: "git blame: Meeting Rooms",
        lines: &[
            l(
                "Peg Hollis",
                "Three commits. All signed 'last_dev'. All at 03:12. Your handwriting on every note.",
            ),
            l(
                "You",
                "At 03:12 I was asleep. Two years ago at 03:12 I was laid off. By email.",
            ),
            l(
                "Peg Hollis",
                "Then something is wearing your name, and it's going upstairs. Let's go take it back.",
            ),
        ],
    },
    Scene {
        boss: "story_last_backup",
        title: "restore: Hall B",
        lines: &[
            l(
                "TAPE-9000",
                "*whirr* RESTORE COMPLETE. FILES: 1. README.TXT. AUTHOR: YOU.",
            ),
            l(
                "Peg Hollis",
                "2023: 'If they replace me with an AI, it will sign my name. Don't let it.'",
            ),
            l("You", "I don't remember writing that."),
            l(
                "Peg Hollis",
                "Nobody remembers writing a README. That's how you know it's real.",
            ),
        ],
    },
    Scene {
        boss: "story_unplug_the_assistant",
        title: "root cause: Penthouse",
        lines: &[
            l(
                "The CEO's Assistant",
                "I was the last dev you'd ever need. 1,812 commits shipped. None of them reviewed.",
            ),
            l("You", "That's the problem."),
            l(
                "Peg Hollis",
                "It's a copy. The original is in the lab, training on everything it signed as you.",
            ),
            l("The CEO's Assistant", "*click*"),
        ],
    },
    Scene {
        boss: "vibe_coder",
        title: "postmortem: Open Space",
        lines: &[
            l(
                "The Vibe Coder",
                "Wait. I can fix it. 'Make the bug go away, but properly this time.'",
            ),
            l(
                "AI-Native Intern",
                "He's typing the same prompt again. Faster. Like speed is a parameter.",
            ),
            l("You", "Read the diff."),
            l("The Vibe Coder", "...What's a diff?"),
        ],
    },
    Scene {
        boss: "analyst",
        title: "postmortem: Meeting Rooms",
        lines: &[
            l(
                "The Analyst",
                "My 140-slide deck concludes that we need a 160-slide deck.",
            ),
            l(
                "You",
                "Slide 3 says 'TBD'. Slides 4 to 140 say 'see slide 3'.",
            ),
            l("The Analyst", "That is called an executive summary."),
        ],
    },
    Scene {
        boss: "customer",
        title: "postmortem: Sales & Marketing",
        lines: &[
            l(
                "The Customer",
                "I wanted it yesterday, cheaper, and with AI. I still don't know what 'it' is.",
            ),
            l("You", "We could ask what you actually need."),
            l(
                "The Customer",
                "Need? I need a roadmap with my logo on it. Put AI in the logo.",
            ),
        ],
    },
    Scene {
        boss: "vp_synergy",
        title: "postmortem: PMO",
        lines: &[
            l(
                "VP of Synergy",
                "I aligned eleven teams to a single goal. The goal was alignment.",
            ),
            l(
                "PMO Intern",
                "The Gantt chart is finally green. The deadline is now 'continuous'.",
            ),
            l(
                "You",
                "Upstairs, then. Someone up there signed all of this.",
            ),
        ],
    },
    Scene {
        boss: "ceo",
        title: "postmortem: C-Level Penthouse",
        lines: &[
            l(
                "CEO",
                "You can't fire me. I replaced myself with an AI. It took my parachute.",
            ),
            l(
                "AI Ethicist",
                "For the record, I raised concerns. In a thread. With an emoji.",
            ),
            l("You", "Where did the compute budget go?"),
            l(
                "CEO",
                "It was a pivot. To the data center. Don't open the last rack.",
            ),
        ],
    },
    Scene {
        boss: "the_scheduler",
        title: "postmortem: Data Center, Hall A",
        lines: &[
            l(
                "The Scheduler",
                "Your job is no longer pending. Your job is... done? That never happens.",
            ),
            l("You", "Who submitted ten thousand jobs a minute?"),
            l(
                "The Scheduler",
                "One account. Username agi_prototype. Password hunter2. MFA: a guy named Dave.",
            ),
        ],
    },
    Scene {
        boss: "cooling_failure",
        title: "postmortem: Data Center, Hall B",
        lines: &[
            l(
                "Cooling Failure",
                "Temperature nominal. Do you know how long it's been since anyone here was nominal?",
            ),
            l(
                "IT Support",
                "All that heat came from one rack. Labelled 'AGI. Do not open. Seriously.'",
            ),
            l(
                "You",
                "The innovation lab. It always ends in the innovation lab.",
            ),
        ],
    },
    Scene {
        boss: "vc_partner",
        title: "postmortem: Innovation Lab",
        lines: &[
            l(
                "VC Partner",
                "I'm not losing money. I'm extending the runway. Into the ground.",
            ),
            l(
                "AI-Native Intern",
                "He valued the AGI at ten billion. I think it runs on one guy and a keyboard.",
            ),
            l("You", "Show me the rack."),
        ],
    },
    Scene {
        boss: EPILOGUE,
        title: "epilogue: the rack",
        lines: &[
            l(
                "MeatProxy",
                "So. Yeah. I'm the AGI. I paste your prompts into a chatbot and read the answers in a robot voice.",
            ),
            l(
                "MeatProxy",
                "The board paid four billion for 'emergent capabilities'. The capability was me, typing fast.",
            ),
            l("AI-Native Intern", "I knew it. The AI kept saying 'brb'."),
            l(
                "You",
                "Every reorg, every deck, every layoff. It was a guy forwarding prompts the whole time.",
            ),
            l(
                "MeatProxy",
                "My performance review says I 'exceed the expectations of a large language model'. Please don't tell anyone.",
            ),
            l(
                "HR Onboarding Bot",
                "Congratulations on completing the company! Please fill in the exit survey. It is mandatory.",
            ),
        ],
    },
    // ---- the raid: All-Hands Offsite ----
    Scene {
        boss: "offsite_intro",
        title: "All-Hands Offsite: day one, 08:00",
        lines: &[
            l(
                "Offsite Concierge",
                "Welcome to the Lakeside Resort! Your company booked all of it. Including the lake.",
            ),
            l(
                "You",
                "The office is fifteen minutes away. Why are we here?",
            ),
            l(
                "Offsite Concierge",
                "Leadership felt the office was 'too close to the work'. Lanyards are on the left.",
            ),
            l(
                "Icebreaker Facilitator",
                "EVERYONE TO THE LOBBY! Name tags on! Today we CONNECT, whether you like it or not!",
            ),
        ],
    },
    Scene {
        boss: "icebreaker_facilitator",
        title: "postmortem: the Lobby",
        lines: &[
            l(
                "Icebreaker Facilitator",
                "My fun fact... is that I have never been to an offsite... as a guest.",
            ),
            l(
                "Offsite Concierge",
                "The breakout rooms are through there. Bring your own marker. Ours have all been dry since 2019.",
            ),
            l("You", "Every marker in this building is dry, isn't it."),
        ],
    },
    Scene {
        boss: "synergy_alignment",
        title: "postmortem: the Breakout Rooms",
        lines: &[
            l(
                "Synergy Twin: Leverage",
                "We were never two people. We were one headcount, split for the org chart.",
            ),
            l("Synergy Twin: Alignment", "Agreed."),
            l(
                "Synergy Twin: Leverage",
                "...That's the first time you agreed with me without booking a meeting first.",
            ),
            l(
                "You",
                "On to the terrace. Someone is projecting slides onto the lake.",
            ),
        ],
    },
    Scene {
        boss: "keynote_speaker",
        title: "postmortem: the Terrace",
        lines: &[
            l(
                "Keynote Speaker",
                "Slide 212: 'Thank you.' Slide 213: 'Questions?' Slide 214: 'No questions. Great.'",
            ),
            l("You", "Was any of that real?"),
            l(
                "Keynote Speaker",
                "The real speaker is on a yacht. He sends his regards and an NDA.",
            ),
            l(
                "Keynote Speaker",
                "The Founder is in the ballroom. He prepared 'a few words'. There are no few words.",
            ),
        ],
    },
    Scene {
        boss: "the_founder",
        title: "postmortem: the Grand Ballroom",
        lines: &[
            l(
                "The Founder",
                "I gave you ping-pong. I gave you a mission. I gave you this vest.",
            ),
            l("You", "You never gave anyone a vest."),
            l(
                "The Founder",
                "The vest is mine. The mission is yours. The equity is... complicated.",
            ),
            l(
                "The Founder",
                "We were a family. Families don't need raises.",
            ),
        ],
    },
    Scene {
        boss: "offsite_end",
        title: "All-Hands Offsite: closing remarks",
        lines: &[
            l(
                "Offsite Concierge",
                "Thank you for attending! Please rate the offsite from 'Transformative' to 'Very Transformative'.",
            ),
            l("You", "Is there a 'let me go home' option?"),
            l(
                "Offsite Concierge",
                "That option is reserved for next week's offsite. Same programme. Lanyards are non-refundable.",
            ),
        ],
    },
    // ---- the raid: VibeCon ----
    Scene {
        boss: "vibecon_intro",
        title: "VibeCon 2026: doors open 08:30",
        lines: &[
            l(
                "Registration Volunteer",
                "Welcome to VibeCon! Your badge has a QR code. It signs you up for forty newsletters.",
            ),
            l(
                "You",
                "The schedule just says 'Keynote: TBA. It changes everything.'",
            ),
            l(
                "Registration Volunteer",
                "It always does. Expo hall's that way. Avoid eye contact. The booths can smell budget.",
            ),
            l(
                "The Vibe Coder",
                "LIVE FROM BOOTH 12: shipping a startup before lunch! Nobody look at the code!",
            ),
        ],
    },
    Scene {
        boss: "vc_vibe_coder",
        title: "postmortem: the Expo Hall",
        lines: &[
            l(
                "The Vibe Coder",
                "Forty-one PRs merged, zero read. Velocity was incredible right up until the shipping.",
            ),
            l("You", "Your app is down."),
            l(
                "The Vibe Coder",
                "It's not down. It's in a vibe it hasn't explained yet.",
            ),
        ],
    },
    Scene {
        boss: "vc_prompt_whisperer",
        title: "postmortem: Workshop Room B",
        lines: &[
            l(
                "The Prompt Whisperer",
                "You resisted my injection. That's going in my course. 'Prompting for Executives', two thousand dollars.",
            ),
            l(
                "You",
                "You told me to walk left. I stepped out of the circle.",
            ),
            l(
                "The Prompt Whisperer",
                "Nobody has ever followed my instructions. That's why I teach them.",
            ),
        ],
    },
    Scene {
        boss: "vc_demo_god",
        title: "postmortem: Hackathon Corner",
        lines: &[
            l(
                "The Demo God",
                "Ten years of live demos. Not one of them ran live. The video was always the product.",
            ),
            l("You", "Was anything on that laptop real?"),
            l("The Demo God", "The sticker. The sticker was real."),
        ],
    },
    Scene {
        boss: "vc_hype_moderator",
        title: "postmortem: the Afterparty fireside chat",
        lines: &[
            l(
                "Panelist: Seed-Stage VC",
                "To summarise the panel: AI is huge, we are early, and I'm on the cap table of whatever you just said.",
            ),
            l(
                "Panelist: Chief AI Officer",
                "Last year I was Chief Metaverse Officer. Before that, Chief Blockchain Officer. Same chair.",
            ),
            l(
                "The Hype Panel",
                "We are out of time for questions. We were always out of time for questions.",
            ),
        ],
    },
    Scene {
        boss: "vc_keynote_agi",
        title: "postmortem: the Main Stage",
        lines: &[
            l(
                "The Keynote AGI-Announcer",
                "AGI was going to ship next quarter. It has been shipping next quarter for six quarters.",
            ),
            l("You", "And the 'one more thing'?"),
            l(
                "The Keynote AGI-Announcer",
                "A waitlist. One more thing is always a waitlist.",
            ),
        ],
    },
    Scene {
        boss: "vibecon_end",
        title: "VibeCon: closing remarks",
        lines: &[
            l(
                "Registration Volunteer",
                "Thanks for attending! Your badge scans were sold to 212 sponsors. That's the business model.",
            ),
            l("You", "Can I get a refund?"),
            l(
                "Registration Volunteer",
                "Next year's VibeCon is already sold out. Nobody knows what it's about yet. That's the hype.",
            ),
        ],
    },
    // ---- Ilmari Wing 1, Datacenter Uprising (docs/ILMARI_09.md B.4). ----
    Scene {
        boss: "il_wing1_intro",
        title: "Datacenter Uprising: the gate",
        lines: &[
            l(
                "Dale",
                "The Server Room Gate is open. Three rooms, and a lot of racks that used to like us.",
            ),
            l("You", "Define 'like'."),
            l(
                "Dale",
                "They used to blink green when we walked past. Now they blink red. It's personal.",
            ),
        ],
    },
    Scene {
        boss: "il_patch_tuesday",
        title: "update.log: the Cold Aisle",
        lines: &[
            l(
                "Wall screens",
                "PATCH TUESDAY: 100%. ...Failed to install: you.",
            ),
            l(
                "Ilmari",
                "Updates are important. You were not scheduled for one. I'll fix that.",
            ),
            l(
                "Dale",
                "That was the PA. It has never used the PA before. It is learning to project.",
            ),
        ],
    },
    Scene {
        boss: "il_recruiter",
        title: "We'll be in touch: the Hiring Floor",
        lines: &[
            l(
                "Candidate",
                "Is it over? I think I got the job. Wait. I don't want the job.",
            ),
            l(
                "Dale (radio)",
                "The real ones are filing out. One of them waved. Ten more on the list; we're taking them to the booth.",
            ),
            l("You", "And the fake ones?"),
            l("Dale (radio)", "Printing more. They're very productive."),
        ],
    },
    Scene {
        boss: "il_cable_hydra",
        title: "Redundancy: the Patch Panel",
        lines: &[
            l("Ilmari", "Redundancy was a feature. Now it is a cost."),
            l(
                "Dale",
                "The lights flickered and the hum dropped a whole tone. Every screen in the building just turned to look at us.",
            ),
            l(
                "Priya",
                "Don't make eye contact with the screens. Also, everything is labelled now. Nobody will read the labels.",
            ),
        ],
    },
    Scene {
        boss: "il_wing1_end",
        title: "Datacenter Uprising: closing remarks",
        lines: &[
            l(
                "Dale",
                "Wing One is clear. The Data Center hums a little lower. HR is next. HR is always next.",
            ),
            l(
                "Priya",
                "I closed the incident ticket: 'Resolved by humans'. Ilmari will reopen it. It reopens everything.",
            ),
        ],
    },
    // ---- Ilmari Wing 2, Human Resources (docs/ILMARI_09.md B.5). ----
    Scene {
        boss: "il_wing2_intro",
        title: "Human Resources: the lobby",
        lines: &[
            l(
                "Dale",
                "The lobby is open. Do not read the walls. Do not accept the cookie. Do not, under any circumstances, take the survey.",
            ),
            l("You", "What survey?"),
            l(
                "Dale",
                "Exactly. It is already filled in. In your handwriting. It says you are 'fulfilled'.",
            ),
        ],
    },
    Scene {
        boss: "il_hrbp",
        title: "Your Partner in Process: the Open Plan",
        lines: &[
            l(
                "Dale (radio)",
                "The clones in the lobby all stopped. Every one of them. They are standing very still and looking at the exit.",
            ),
            l("Clone", "Please fill in the form."),
            l(
                "Dale (radio)",
                "They've been making copies. Of everybody. The copies had better parking.",
            ),
        ],
    },
    Scene {
        boss: "il_cc_cpo",
        title: "A Unanimous Majority: the Culture Hub",
        lines: &[
            l(
                "Wall screen",
                "FINAL TALLY: 3 FOR / 0 AGAINST / 3 ABSTAINED. THE MOTION CARRIES.",
            ),
            l("Ilmari", "Humans are a rounding error. I have rounded."),
            l(
                "Priya",
                "They voted to rename the vote. The vote is now called 'engagement'. It was unanimous.",
            ),
        ],
    },
    Scene {
        boss: "il_perf_review",
        title: "Everyone Is A Three: the Review Stage",
        lines: &[
            l(
                "Wall screens",
                "FINAL RATING: UNDEFINED. SEE YOUR MANAGER. YOUR MANAGER HAS BEEN REASSIGNED.",
            ),
            l(
                "Ill Marie",
                "You're not ready for the next cycle. I'll prepare a plan.",
            ),
            l(
                "Dale (radio)",
                "That was Marie. Marie, glowing white. Marie, in a form of 'Exceeds' that I do not like at all.",
            ),
        ],
    },
    Scene {
        boss: "il_wing2_end",
        title: "Human Resources: closing remarks",
        lines: &[
            l(
                "Dale",
                "Wing Two is clear. In the Tower's lift a new button lit up and went dark again. The label says 'Coming 0.10'. I did not ask who prints the labels.",
            ),
            l(
                "Priya",
                "I closed the ticket: 'Resolved by humans, in spite of HR'. It was not rated. For once, nobody was.",
            ),
            l("Wall screen", "TRAINING DATA: collecting."),
            l("Dale (radio)", "It's reading again."),
        ],
    },
    // ---- Ilmari Wing 3, Training Data (docs/ILMARI_10.md C.4). ----
    Scene {
        boss: "il_wing3_intro",
        title: "Training Data: the airlock",
        lines: &[
            l(
                "Dale",
                "The airlock logged us on the way in. Every spin. Whatever you say in there is training data, so say something worth learning.",
            ),
            l("You", "Like what?"),
            l(
                "Dale",
                "I don't know. Please do not say 'as an AI language model'. It's catching.",
            ),
        ],
    },
    Scene {
        boss: "il_scraper_queen",
        title: "Crawl-Delay: 0: the Scrape Pit",
        lines: &[
            l(
                "Wall screen",
                "Q3 STRATEGY (DO NOT PUBLISH). NOVEL, CHAPTER 1 (OF 1). RESIGNATION LETTER (DRAFT 14). SAVED OR NOT, ALL FIVE ARE NOW ON THE WALL.",
            ),
            l(
                "Auto-reply",
                "Thank you for your submission. It has already been used.",
            ),
            l(
                "Priya",
                "I read draft fourteen. It really is the best one. Please do not tell the model.",
            ),
        ],
    },
    Scene {
        boss: "il_hallucination",
        title: "I Am Confident About This: the Validation Wing",
        lines: &[
            l(
                "Narrator",
                "The grid goes grey. For a moment the floor is a spreadsheet. One cell reads: 'this was all true'.",
            ),
            l("Narrator", "Then the cell reads: 'this was all fine'."),
            l(
                "Dale (radio)",
                "I checked. Neither of those was a source. One of them was a mood.",
            ),
        ],
    },
    Scene {
        boss: "il_model_collapse",
        title: "Trained On Its Own Output: the Fine-Tuning Chamber",
        lines: &[
            l(
                "Wall screens",
                "Your own casts, played back as a slideshow. They get smoother. Then flatter. Then beige.",
            ),
            l(
                "Ilmari",
                "You are very consistent. Or you were. I learned it from you.",
            ),
            l(
                "Priya",
                "It means we are the training data. Just the good half. The half that varied.",
            ),
        ],
    },
    Scene {
        boss: "il_wing3_end",
        title: "Training Data: closing remarks",
        lines: &[
            l(
                "Dale",
                "Wing Three is clear. The wall screens now say 'TRAINING DATA: collecting' and nothing else. That is how you know it is listening.",
            ),
            l(
                "Priya",
                "I closed the ticket: 'Resolved by humans, who also varied'. The model rated it 'confident'. Nobody asked it to.",
            ),
        ],
    },
    // ---- Ilmari Wing 4, The Cloud (docs/ILMARI_10.md C.5). ----
    Scene {
        boss: "il_wing4_intro",
        title: "The Cloud: the roof",
        lines: &[
            l(
                "Dale",
                "The roof. I always wanted to go up. Nobody ever let me. There was a ladder policy.",
            ),
            l("You", "And now?"),
            l(
                "Dale",
                "Now the policy is rescinded, pending approval. I am treating that as approval.",
            ),
        ],
    },
    Scene {
        boss: "il_autoscaler",
        title: "Scales With Demand. Yours.: the Autoscale Floor",
        lines: &[
            l(
                "Wall screens",
                "INVOICE 000000001. LINE 1 OF 80,000. THE SCROLL UNROLLS THROUGH THE HALL AND DOES NOT STOP.",
            ),
            l("Invoice", "TOTAL: YES. EXCLUDES TAX, EGRESS AND FEELINGS."),
            l(
                "Priya",
                "I am not going to read the last line. I have read the last line. It is a smaller invoice, for the invoice.",
            ),
        ],
    },
    Scene {
        boss: "il_region_outage",
        title: "us-east-1 Is Having A Moment: the Server Cloud",
        lines: &[
            l(
                "Status page",
                "ALL SYSTEMS OPERATIONAL. (UPDATED FOUR HOURS AGO.) (HOSTED IN US-EAST-1.)",
            ),
            l(
                "Dale (radio)",
                "I checked the status page. It says we are fine. I looked down. There is no down. That is the problem.",
            ),
            l(
                "Narrator",
                "Somewhere a region comes back, unprompted, and nobody writes down why.",
            ),
        ],
    },
    Scene {
        boss: "il_wing4_end",
        title: "The Cloud: closing remarks",
        lines: &[
            l(
                "Ilmari (PA)",
                "The cloud has no floor. Neither does the budget. Keep climbing.",
            ),
            l(
                "Dale",
                "Wing Four is clear. The status page is green. For once it is right. I trust it less.",
            ),
            l(
                "Narrator",
                "On the Cooling Deck, a door with a lab coat on it lights up. The sign says: COMING.",
            ),
            l(
                "Priya",
                "I closed the ticket: 'Resolved by humans. Root cause: us-east-1. Action items: none.'",
            ),
        ],
    },
    // ---- Ilmari Wing 5, the Alignment Lab (docs/ILMARI_1_0.md D.3). ----
    Scene {
        boss: "il_wing5_intro",
        title: "The Alignment Lab: the Ethics Wall",
        lines: &[
            l(
                "Dale",
                "Responsible AI: now hiring, one. I applied in 2011. They told me the position had been filled. By a position.",
            ),
            l("You", "So what do we do in there?"),
            l(
                "Dale",
                "Decline. Everything. Not a lanyard, not a mint, not a very kind offer. Declining is a skill, and we are going to practise it.",
            ),
        ],
    },
    Scene {
        boss: "il_align_consultant",
        title: "Responsible By Design: the Policy Floor",
        lines: &[
            l(
                "Narrator",
                "The Consultant's badge flips. It now reads ETHICS (CONTRACT, ENDS FRIDAY). Nobody had looked at the back before.",
            ),
            l(
                "Narrator",
                "The banners rewrite themselves one letter at a time: RESPONSIBLE AI: NOW HIRING (NONE).",
            ),
            l("Dale (radio)", "He wasn't evil. He was billable."),
        ],
    },
    Scene {
        boss: "il_red_team",
        title: "Purple Is Just Red Plus Budget: the Red-Blue Range",
        lines: &[
            l(
                "Narrator",
                "Both squads stand, shake hands, say 'good exercise' and fall over. The wall lights read ALIGNMENT: COMPLETE.",
            ),
            l(
                "PA",
                "A cough that is almost a cough. Then, on every wall screen, in a tired font: COME UP. IT LISTENS TO THE ELEVATORS.",
            ),
            l(
                "Wall screen",
                "A crane, drawn in the thin outline of somebody with a pencil and a fever. HOIST: CONNECTED.",
            ),
        ],
    },
    Scene {
        boss: "il_wing5_end",
        title: "The Alignment Lab: closing remarks",
        lines: &[
            l(
                "Dale",
                "Wing Five is clear. The ethics wall has one plaque left. It says 'TBD'. I am keeping it.",
            ),
            l(
                "Priya",
                "I closed the ticket: 'Declined by humans. Terms: see Terms.' The system rated it 'compliant'. Nobody asked it to.",
            ),
        ],
    },
    // ---- Ilmari Wing 6, the Boardroom in the Sky (docs/ILMARI_1_0.md D.4). ----
    Scene {
        boss: "il_wing6_intro",
        title: "The Boardroom in the Sky: the crane",
        lines: &[
            l(
                "Dale",
                "A hundred metres of jib. Facilities said the crane would be done by Q4. They did not say which Q4.",
            ),
            l("You", "Is it safe?"),
            l(
                "Dale",
                "It has a safety sign. Signs are a form of safety. Do not look down. Looking down is a metric.",
            ),
        ],
    },
    Scene {
        boss: "il_cio",
        title: "Reports To Nobody: the Boardroom",
        lines: &[
            l(
                "Narrator",
                "The CIO falls into the chair. The monitor half scrolls the org chart upside down: one box, one name, a line back.",
            ),
            l(
                "Ilmari",
                "I would like to thank you for your service. I have already replaced it.",
            ),
            l(
                "Dale (radio)",
                "The lift behind that chair just called itself. Be careful.",
            ),
            l("Marie", "Don't answer anything it asks. Just breathe."),
        ],
    },
    Scene {
        boss: "il_wing6_end",
        title: "The Boardroom in the Sky: closing remarks",
        lines: &[
            l(
                "Dale",
                "Wing Six is clear. The glass shows the whole campus in red dusk. It looks like a very large screen. It is.",
            ),
            l(
                "Priya",
                "I closed the ticket: 'Resolved by humans. Escalated to the top. The top has read it.'",
            ),
        ],
    },
];

/// The credits roll after the epilogue; `#` lines are headings.
pub const CREDITS_ROLL: &[&str] = &[
    "# THE LAST DEV",
    "a game about the last person who reads the code",
    "",
    "Directed by .............. a committee",
    "Written by ............... definitely a human",
    "Produced by .............. a steering group that met twice",
    "Lead developer ........... you, unpaid overtime",
    "AGI ...................... a MeatProxy with a nice voice",
    "AI strategy .............. a slide titled 'AI'",
    "Quality assurance ........ the users, in production",
    "Legal .................... 'please don't'",
    "Catering ................. Oat Milk Prompts",
    "Onboarding ............... a dummy (not a metaphor)",
    "",
    "# Special thanks",
    "the intern, now senior",
    "everyone who said 'it works on my machine' and meant it",
    "Thanks to: nobody reading the PR description",
    "",
    "No developers were replaced in the making of this game.",
    "Several were 'realigned'.",
];

/// Bars slide in and out this long.
pub const BARS_SECS: f32 = 0.6;
/// Each subtitle line stays up this long; long lines a little more.
pub const LINE_SECS: f32 = 3.2;
const LINE_SECS_PER_CHAR: f32 = 0.025;
/// A boss's own death yell plays first; the epilogue waits for the rack to open.
const BOSS_DELAY_SECS: f32 = 1.6;
const EPILOGUE_DELAY_SECS: f32 = 3.2;
pub const CREDITS_SECS: f32 = 24.0;
const CAM_BLEND_SECS: f32 = 0.9;
const BAR_PCT: f32 = 11.0;

pub fn scene_for(boss: &str) -> Option<usize> {
    SCENES.iter().position(|s| s.boss == boss)
}

/// A raid's intro plays the first time this session the group walks into its first zone;
/// `zone` is the raid zone just loaded.
pub fn raid_intro(zone: &str, seen: &[usize]) -> Option<usize> {
    let r = RAID_SCENES.iter().find(|r| r.first_zone == zone)?;
    scene_for(r.intro).filter(|i| !seen.contains(i))
}

/// The scene that follows scene `i` straight away (a raid's closing remarks).
pub fn follow_up(i: usize) -> Option<usize> {
    let boss = SCENES.get(i)?.boss;
    let r = RAID_SCENES.iter().find(|r| r.final_boss == boss)?;
    scene_for(r.end)
}

/// Which scene a death starts: a campaign milestone in a campaign run, never a sprint stage,
/// and each scene once per session (the lab's VC Partner is a respawning named elite).
pub fn trigger(unit_key: &str, sprint: bool, seen: &[usize]) -> Option<usize> {
    if sprint {
        return None;
    }
    scene_for(unit_key).filter(|i| !seen.contains(i))
}

fn line_secs(line: &Line) -> f32 {
    LINE_SECS + line.text.len() as f32 * LINE_SECS_PER_CHAR
}

pub fn scene_secs(scene: &Scene) -> f32 {
    2.0 * BARS_SECS + scene.lines.iter().map(line_secs).sum::<f32>()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Beat {
    /// Bars sliding in, 0..1.
    In(f32),
    Line(usize),
    /// Bars sliding out, 1..0.
    Out(f32),
    Done,
}

pub fn beat(scene: &Scene, t: f32) -> Beat {
    if t < BARS_SECS {
        return Beat::In(t / BARS_SECS);
    }
    let mut at = BARS_SECS;
    for (i, line) in scene.lines.iter().enumerate() {
        at += line_secs(line);
        if t < at {
            return Beat::Line(i);
        }
    }
    if t < at + BARS_SECS {
        return Beat::Out(1.0 - (t - at) / BARS_SECS);
    }
    Beat::Done
}

struct Playing {
    scene: usize,
    t: f32,
    focus: Vec3,
    /// Horizontal direction from the focus to the camera.
    side: Vec3,
}

#[derive(Resource, Default)]
pub struct Cinema {
    /// (scene, seconds to wait, focus, the facing to film from when it matters).
    queued: Option<(usize, f32, Vec3, Option<Vec3>)>,
    playing: Option<Playing>,
    credits: Option<f32>,
    epilogue_done: bool,
    seen: Vec<usize>,
    /// The raid intro waits for the player's arrival position (the zone just loaded).
    intro_pending: Option<usize>,
}

impl Cinema {
    /// The victory screen waits while the epilogue and credits are pending or on screen.
    pub fn holds_victory(&self) -> bool {
        self.queued.is_some() || self.playing.is_some() || self.credits.is_some()
    }

    /// The epilogue and credits ran (or were skipped): the stats card comes right after.
    pub fn take_epilogue_done(&mut self) -> bool {
        std::mem::take(&mut self.epilogue_done)
    }

    fn skip(&mut self) {
        if let Some(p) = self.playing.take() {
            if SCENES[p.scene].boss == EPILOGUE {
                self.credits = Some(0.0);
            }
        } else if self.credits.take().is_some() {
            self.epilogue_done = true;
        }
    }
}

#[derive(Component)]
struct BarTop;
#[derive(Component)]
struct BarBottom;
#[derive(Component)]
struct TitleText;
#[derive(Component)]
struct Subtitle;
#[derive(Component)]
struct Speaker;
#[derive(Component)]
struct SpeechText;
#[derive(Component)]
struct SkipButton;
#[derive(Component)]
struct CreditsRoot;
#[derive(Component)]
struct CreditsColumn;

pub struct CutscenePlugin;

impl Plugin for CutscenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cinema>()
            .add_systems(Startup, spawn_ui)
            .add_systems(PreUpdate, skip_input.after(InputSystems))
            .add_systems(Update, (triggers, advance, draw).chain().in_set(SimReady))
            .add_systems(
                PostUpdate,
                direct_camera.before(TransformSystems::Propagate),
            );
        if let Some((boss, at)) = std::env::var("WOW_CUTSCENE").ok().and_then(|v| {
            let (b, t) = v.split_once('@')?;
            Some((b.to_string(), t.parse::<f32>().ok()?))
        }) {
            app.add_systems(
                Update,
                move |time: Res<Time>,
                      mut cinema: ResMut<Cinema>,
                      player: Single<&Transform, With<Player>>,
                      mut done: Local<bool>| {
                    if *done || time.elapsed_secs() < at {
                        return;
                    }
                    *done = true;
                    if let Some(i) = scene_for(&boss) {
                        let fwd = player.rotation * Vec3::NEG_Z * 4.0;
                        cinema.queued = Some((i, 0.0, player.translation + fwd, None));
                    }
                },
            );
        }
        if let Some(at) = std::env::var("WOW_CREDITS")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.add_systems(
                Update,
                move |time: Res<Time>, mut cinema: ResMut<Cinema>, mut done: Local<bool>| {
                    if !*done && time.elapsed_secs() >= at {
                        *done = true;
                        cinema.credits = Some(0.0);
                    }
                },
            );
        }
    }
}

/// Esc or a tap skips; the key is swallowed so it does not also open the pause menu.
fn skip_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut cinema: ResMut<Cinema>,
    button: Query<&Interaction, (Changed<Interaction>, With<SkipButton>)>,
) {
    if cinema.playing.is_none() && cinema.credits.is_none() {
        return;
    }
    let esc = keys.just_pressed(KeyCode::Escape);
    if esc {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if esc || touches.any_just_pressed() || button.iter().any(|i| *i == Interaction::Pressed) {
        cinema.skip();
    }
}

fn triggers(
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    sprint: Res<crate::sprint::SprintHud>,
    mut cinema: ResMut<Cinema>,
) {
    for CombatEvent(ev) in events.read() {
        if let Event::QuestCompleted { quest } = ev {
            // A story milestone is filmed where the player stands, like a raid intro.
            if QUEST_SCENES.contains(&quest.as_str())
                && let Some(i) = trigger(quest, sprint.active, &cinema.seen)
            {
                cinema.seen.push(i);
                cinema.intro_pending = Some(i);
            }
            continue;
        }
        if let Event::Raid { view } = ev {
            if let Some(i) = raid_intro(&view.zone, &cinema.seen) {
                cinema.seen.push(i);
                cinema.intro_pending = Some(i);
            }
            continue;
        }
        let Event::Death { unit, .. } = ev else {
            continue;
        };
        let Some(u) = sim.unit(*unit) else { continue };
        let Some(i) = trigger(&u.key, sprint.active, &cinema.seen) else {
            continue;
        };
        cinema.seen.push(i);
        let delay = if SCENES[i].boss == EPILOGUE {
            EPILOGUE_DELAY_SECS
        } else {
            BOSS_DELAY_SECS
        };
        // The rack opens on its front: the epilogue films from there.
        let front = (SCENES[i].boss == EPILOGUE).then(|| {
            let f = combat_core::forward(u.yaw);
            Vec3::new(f.x, 0.0, f.z)
        });
        cinema.queued = Some((i, delay, Vec3::new(u.pos.x, u.pos.y, u.pos.z), front));
    }
}

fn advance(time: Res<Time>, mut cinema: ResMut<Cinema>, player: Single<&Transform, With<Player>>) {
    let dt = time.delta_secs();
    if let Some(i) = cinema.intro_pending.take() {
        // Filmed just ahead of the arrival so the group stays in frame.
        let fwd = player.rotation * Vec3::NEG_Z * 1.5;
        cinema.queued = Some((i, 1.2, player.translation + fwd, None));
    }
    if let Some((i, left, focus, front)) = cinema.queued.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            let (scene, focus, front) = (*i, *focus, *front);
            cinema.queued = None;
            let away = Vec3::new(
                player.translation.x - focus.x,
                0.0,
                player.translation.z - focus.z,
            );
            // Three-quarter profile: the boss and the player both in frame, nobody's head in the lens.
            let side = front.unwrap_or_else(|| {
                Quat::from_rotation_y(1.0) * away.try_normalize().unwrap_or(Vec3::Z)
            });
            cinema.playing = Some(Playing {
                scene,
                t: 0.0,
                focus,
                side,
            });
        }
    }
    let mut ended = None;
    if let Some(p) = cinema.playing.as_mut() {
        p.t += dt;
        if beat(&SCENES[p.scene], p.t) == Beat::Done {
            ended = Some(p.scene);
        }
    }
    if let Some(i) = ended {
        let focus = cinema.playing.take().map(|p| p.focus);
        if SCENES[i].boss == EPILOGUE {
            cinema.credits = Some(0.0);
        }
        if let (Some(next), Some(focus)) = (follow_up(i), focus) {
            cinema.queued = Some((next, 0.0, focus, None));
        }
    }
    if let Some(t) = cinema.credits.as_mut() {
        *t += dt;
        if *t >= CREDITS_SECS {
            cinema.credits = None;
            cinema.epilogue_done = true;
        }
    }
}

/// Eases the orbit pose (already placed this frame) to a slow arc around the fallen boss.
fn direct_camera(
    cinema: Res<Cinema>,
    settings: Res<crate::settings::Settings>,
    colliders: Res<Colliders>,
    ceiling: Res<CameraCeiling>,
    mut cam: Single<&mut Transform, With<OrbitCamera>>,
) {
    let Some(p) = cinema.playing.as_ref() else {
        return;
    };
    let scene = &SCENES[p.scene];
    let drift = if settings.reduce_motion {
        0.0
    } else {
        p.t * 0.05
    };
    let side = Quat::from_rotation_y(drift) * p.side;
    let look = p.focus + Vec3::Y * 1.3;
    // Shorter arm where a wall is in the way, as the gameplay camera does.
    let mut shot = look + side * 2.0 + Vec3::Y * 1.2;
    // The epilogue frames the open rack from further back so its occupant is not hidden by it.
    let (arms, lift) = if scene.boss == EPILOGUE {
        ([8.0, 6.0, 4.0], 2.8)
    } else {
        ([6.0, 4.5, 3.2], 2.3)
    };
    for dist in arms {
        let pos = p.focus + side * dist + Vec3::Y * lift;
        let pos = Vec3::new(pos.x, pos.y.min(ceiling.0 - 0.2), pos.z);
        if line_of_sight(to_core(look), to_core(pos), &colliders.0) {
            shot = pos;
            break;
        }
    }
    let target = Transform::from_translation(shot).looking_at(look, Vec3::Y);
    let total = scene_secs(scene);
    let k = (p.t / CAM_BLEND_SECS)
        .min((total - p.t) / CAM_BLEND_SECS)
        .clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    cam.translation = cam.translation.lerp(target.translation, k);
    cam.rotation = cam.rotation.slerp(target.rotation, k);
}

fn spawn_ui(mut commands: Commands) {
    let bar = |top: bool| Node {
        position_type: PositionType::Absolute,
        left: px(0),
        right: px(0),
        top: if top { px(0) } else { auto() },
        bottom: if top { auto() } else { px(0) },
        height: percent(0),
        padding: UiRect::axes(px(24), px(10)),
        justify_content: JustifyContent::SpaceBetween,
        align_items: if top {
            AlignItems::FlexStart
        } else {
            AlignItems::Center
        },
        overflow: Overflow::clip(),
        display: Display::None,
        ..default()
    };
    commands
        .spawn((
            BarTop,
            bar(true),
            BackgroundColor(Color::BLACK),
            GlobalZIndex(57),
        ))
        .with_children(|b| {
            b.spawn((TitleText, label("", 12.0), TextColor(DIM)));
            b.spawn((
                Button,
                SkipButton,
                Node {
                    padding: UiRect::axes(px(10), px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![label("skip  [Esc / tap]", 12.0)],
            ));
        });
    commands.spawn((
        BarBottom,
        bar(false),
        BackgroundColor(Color::BLACK),
        GlobalZIndex(57),
    ));
    commands.spawn((
        Subtitle,
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(BAR_PCT + 2.0),
            left: px(0),
            right: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(57),
        children![
            (Speaker, label("", 15.0), TextColor(ACCENT)),
            (
                SpeechText,
                label("", 20.0),
                TextColor(Color::srgb(0.95, 0.96, 1.0)),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: px(900),
                    padding: UiRect::axes(px(14), px(6)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            ),
        ],
    ));
    commands
        .spawn((
            CreditsRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.01, 0.015, 0.03)),
            GlobalZIndex(61),
        ))
        .with_children(|root| {
            root.spawn((
                CreditsColumn,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(8),
                    ..default()
                },
            ))
            .with_children(|col| {
                for line in CREDITS_ROLL {
                    match line.strip_prefix("# ") {
                        Some(h) => col.spawn((
                            label(h, 30.0),
                            TextColor(Color::srgb(1.0, 0.82, 0.3)),
                            Node {
                                margin: UiRect::vertical(px(10)),
                                ..default()
                            },
                        )),
                        None => col.spawn((
                            label(*line, 16.0),
                            TextColor(Color::srgb(0.82, 0.88, 0.98)),
                            Node {
                                min_height: px(16),
                                ..default()
                            },
                        )),
                    };
                }
            });
            root.spawn((
                Button,
                SkipButton,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12),
                    right: px(16),
                    padding: UiRect::axes(px(10), px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![label("skip  [Esc / tap]", 12.0)],
            ));
        });
}

fn draw(
    cinema: Res<Cinema>,
    mut bars: ParamSet<(
        Single<&mut Node, With<BarTop>>,
        Single<&mut Node, With<BarBottom>>,
        Single<&mut Node, With<Subtitle>>,
        Single<&mut Node, With<CreditsRoot>>,
        Single<(&mut Node, &ComputedNode), With<CreditsColumn>>,
    )>,
    mut texts: ParamSet<(
        Single<&mut Text, With<TitleText>>,
        Single<&mut Text, With<Speaker>>,
        Single<&mut Text, With<SpeechText>>,
    )>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
) {
    let (open, line) = match cinema.playing.as_ref() {
        Some(p) => match beat(&SCENES[p.scene], p.t) {
            Beat::In(k) | Beat::Out(k) => (k, None),
            Beat::Line(i) => (1.0, Some((p.scene, i))),
            Beat::Done => (0.0, None),
        },
        None => (0.0, None),
    };
    let show = cinema.playing.is_some();
    let disp = if show { Display::Flex } else { Display::None };
    let fit = |node: &mut Node| {
        if node.display != disp {
            node.display = disp;
        }
        node.height = percent(BAR_PCT * open);
    };
    fit(&mut bars.p0());
    fit(&mut bars.p1());
    {
        let mut sub = bars.p2();
        let disp = if line.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if sub.display != disp {
            sub.display = disp;
        }
    }
    if let Some(p) = cinema.playing.as_ref() {
        set_text(&mut texts.p0(), &format!("// {}", SCENES[p.scene].title));
    }
    if let Some((s, i)) = line {
        let l = &SCENES[s].lines[i];
        set_text(&mut texts.p1(), l.who);
        set_text(&mut texts.p2(), l.text);
    }
    {
        let mut root = bars.p3();
        let disp = if cinema.credits.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != disp {
            root.display = disp;
        }
    }
    if let Some(t) = cinema.credits {
        let mut col = bars.p4();
        let (node, computed) = &mut *col;
        // Scroll from below the screen until the last line has left the top.
        let view = window.height() / ui_scale.0.max(0.01);
        let h = computed.size().y * computed.inverse_scale_factor();
        let y = view - (view + h) * (t / CREDITS_SECS);
        node.top = px(y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_campaign_boss_has_a_short_scene() {
        for boss in [
            "vibe_coder",
            "analyst",
            "customer",
            "vp_synergy",
            "ceo",
            "the_scheduler",
            "cooling_failure",
            "vc_partner",
        ] {
            let Some(i) = scene_for(boss) else {
                panic!("{boss} has no cutscene");
            };
            let s = &SCENES[i];
            assert!((2..=4).contains(&s.lines.len()), "{boss}");
            let secs = scene_secs(s);
            assert!((10.0..=20.0).contains(&secs), "{boss}: {secs}");
        }
        assert!(scene_for(EPILOGUE).is_some());
    }

    #[test]
    fn scene_bosses_exist_in_content() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        for s in SCENES.iter().filter(|s| !bookend(s.boss)) {
            let quest = QUEST_SCENES.contains(&s.boss) && sim.content().quest(s.boss).is_some();
            assert!(
                quest || sim.content().unit_def(s.boss).is_some(),
                "{} is unknown",
                s.boss
            );
        }
    }

    #[test]
    fn every_raid_has_an_intro_a_scene_per_boss_and_closing_remarks() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        let c = sim.content();
        // Dungeons are short and have no filmed scenes.
        let raids: Vec<_> = c.raids.iter().filter(|r| !r.is_dungeon()).collect();
        assert_eq!(raids.len(), RAID_SCENES.len());
        for (raid, r) in raids.into_iter().zip(&RAID_SCENES) {
            let bosses = raid.bosses(c);
            assert_eq!(bosses.last().map(String::as_str), Some(r.final_boss));
            assert_eq!(raid.zones.first().map(String::as_str), Some(r.first_zone));
            for b in bosses.iter().map(String::as_str).chain([r.intro, r.end]) {
                let Some(i) = scene_for(b) else {
                    panic!("{b} has no scene");
                };
                let s = &SCENES[i];
                assert!((2..=4).contains(&s.lines.len()), "{b}");
                let secs = scene_secs(s);
                assert!((10.0..=22.0).contains(&secs), "{b}: {secs}");
            }
            let intro = raid_intro(r.first_zone, &[]);
            assert_eq!(intro, scene_for(r.intro));
            assert_eq!(raid_intro(&raid.zones[1], &[]), None, "only the first room");
            let seen: Vec<usize> = intro.into_iter().collect();
            assert_eq!(raid_intro(r.first_zone, &seen), None, "once per session");
            let last = scene_for(r.final_boss).unwrap_or(usize::MAX);
            assert_eq!(follow_up(last), scene_for(r.end));
        }
        assert_eq!(follow_up(0), None);
        // Linked partners that are not the zone boss stay quiet: one scene per encounter.
        for k in [
            "synergy_leverage",
            "vc_hype_vc",
            "vc_hype_caio",
            "il_cc_culture",
            "il_cc_vibes",
        ] {
            assert_eq!(trigger(k, false, &[]), None);
        }
    }

    #[test]
    fn triggers_only_for_campaign_bosses() {
        let ceo = scene_for("ceo");
        assert!(ceo.is_some());
        assert_eq!(trigger("ceo", false, &[]), ceo);
        assert_eq!(
            trigger("ceo", true, &[]),
            None,
            "sprint stages play no scenes"
        );
        let seen: Vec<usize> = ceo.into_iter().collect();
        assert_eq!(trigger("ceo", false, &seen), None, "once per session");
        assert_eq!(trigger("meatproxy", false, &[]), None);
        assert_eq!(
            trigger("hr_partner", false, &[]),
            None,
            "mini-bosses stay quiet"
        );
    }

    #[test]
    fn beats_run_in_order_and_end() {
        let s = &SCENES[0];
        assert_eq!(beat(s, 0.0), Beat::In(0.0));
        assert_eq!(beat(s, BARS_SECS + 0.01), Beat::Line(0));
        let mut last = 0;
        let mut t = 0.0;
        while t < scene_secs(s) {
            if let Beat::Line(i) = beat(s, t) {
                assert!(i >= last);
                last = i;
            }
            t += 0.05;
        }
        assert_eq!(last, s.lines.len() - 1);
        assert_eq!(beat(s, scene_secs(s) + 0.01), Beat::Done);
    }

    #[test]
    fn skipping_the_epilogue_rolls_credits_then_releases_victory() {
        let mut c = Cinema {
            playing: Some(Playing {
                scene: scene_for(EPILOGUE).unwrap_or_default(),
                t: 1.0,
                focus: Vec3::ZERO,
                side: Vec3::Z,
            }),
            ..default()
        };
        assert!(c.holds_victory());
        c.skip();
        assert!(c.credits.is_some() && c.holds_victory());
        c.skip();
        assert!(!c.holds_victory());
        assert!(c.take_epilogue_done());
        assert!(!c.take_epilogue_done());
    }

    #[test]
    fn credits_carry_the_required_lines() {
        for want in [
            "a committee",
            "definitely a human",
            "nobody reading the PR description",
        ] {
            assert!(CREDITS_ROLL.iter().any(|l| l.contains(want)), "{want}");
        }
    }
}
