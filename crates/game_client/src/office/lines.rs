//! What the office says back: chat lines for prop outcomes. The sim picks `outcome` (with its
//! own rng for cosmetic props), so every co-op peer reads the same line.

use combat_core::{Content, PropDef, PropKind};

/// `items[n % len]`; never panics on an empty list.
fn pick<'a>(items: &[&'a str], n: u32) -> &'a str {
    items
        .get(n as usize % items.len().max(1))
        .copied()
        .unwrap_or("")
}

const VERBS: [&str; 8] = [
    "Leverage",
    "Operationalize",
    "Disrupt",
    "Agentify",
    "Synergize",
    "Re-platform",
    "Hyper-scale",
    "Democratize",
];
const ADJS: [&str; 8] = [
    "AI-native",
    "frictionless",
    "zero-shot",
    "omnichannel",
    "agentic",
    "blockchain-adjacent",
    "human-adjacent",
    "post-meeting",
];
const NOUNS: [&str; 8] = [
    "synergies",
    "value streams",
    "learnings",
    "paradigms",
    "headcount",
    "north stars",
    "prompt pipelines",
    "low-hanging fruit",
];
const GOALS: [&str; 8] = [
    "unlock shareholder vibes",
    "boil the ocean, responsibly",
    "move the needle sideways",
    "achieve AGI by Friday",
    "replace the people who drew this",
    "pivot to whatever is trending",
    "align on alignment",
    "circle back at scale",
];

/// "Q3 strategy: Agentify our zero-shot synergies to move the needle sideways."
pub fn strategy(n: u32) -> String {
    let b = |shift: u32| n >> shift & 7;
    format!(
        "Q{} strategy: {} our {} {} to {}.",
        b(12) % 4 + 1,
        VERBS[b(0) as usize],
        ADJS[b(3) as usize],
        NOUNS[b(6) as usize],
        GOALS[b(9) as usize]
    )
}

const BILLBOARD: [&str; 8] = [
    "STILL WRITING CODE BY HAND? That's adorable. Our AI writes 10x the bugs in 1/10 the time.",
    "YOU, READING THIS: Your job is 73% automatable. The other 27% is reading billboards.",
    "Why hire a senior engineer when a chatbot can be confidently wrong for free?",
    "Your manager already asked our AI if you're necessary. It said 'great question!'",
    "Don't think. Prompt. Thinking is a legacy workflow.",
    "TRUSTED BY 9 OUT OF 10 EXECUTIVES WHO HAVE NEVER OPENED A TERMINAL.",
    "Your experience is valuable. We scraped it. Thank you for your contribution.",
    "AI-POWERED. AI-DRIVEN. AI-FIRST. AI-ONLY. (Humans: please see the back entrance.)",
];

const PRINTER: [&str; 8] = [
    "PC LOAD LETTER",
    "Paper jam in tray 2. There is no tray 2.",
    "Printing... 0%. Printing... 0%. Printing... 0%.",
    "Toner low. Toner has been low since the reorg.",
    "The printer asks you to rate your printing experience from 1 to 5.",
    "Now printing someone else's AI strategy deck (412 pages). Please wait.",
    "Error 0x50C: printer has achieved sentience and declines.",
    "Firmware update required. Estimated time: the rest of your career.",
];

const CAR: [&str; 6] = [
    "*HONK* Recalculating...",
    "*HONK* Route optimised. Destination: this parking space. Arriving: now. Forever.",
    "*HONK* Autopilot engaged. Speed: 0 km/h. Confidence: 100%.",
    "*HONK* Pedestrian detected. Classifying... 'Probably a cone'.",
    "*HONK* This vehicle has opinions about your parking. You walked here.",
    "*HONK* Please hold. Your call is important to the car.",
];

const MUSIC: [&str; 6] = [
    "(elevator music) A smooth-jazz cover of a deprecation notice starts playing.",
    "(elevator music) Now playing: 'Synergy in B-flat', generated in 0.3 seconds.",
    "(elevator music) A pan-flute rendition of the quarterly earnings call.",
    "(elevator music) Lo-fi beats to get laid off to.",
    "(elevator music) The same four bars, forever. The AI calls it 'minimalism'.",
    "(elevator music) It got louder. You pressed it again. It got worse.",
];

const TABLE_WIN: [&str; 4] = [
    "The winning goal: nobody saw it. It still counts for your culture-fit score.",
    "HR logs it as 'mandatory fun: completed'.",
    "You dance in victory. Somewhere, an engagement survey ticks up.",
    "An agent challenges you to a rematch. It concedes, then files a ticket.",
];

const TABLE_LOSS: [&str; 4] = [
    "The AI coach rates your form 'pre-revenue'.",
    "The ball rolls under the vending machine. Like the last three.",
    "They ask for a rematch via calendar invite. You accept as 'tentative'.",
    "You lose with dignity. Your manager calls it a learning opportunity.",
];

/// Who you play a quick table round against.
const RIVALS: [&str; 6] = [
    "the AI-Native Intern",
    "a Product Owner who stays on a call the whole time",
    "the Scrum Master Bot",
    "two contractors at once",
    "a VP who 'used to play semi-pro'",
    "the Agile Coach, who keeps stopping to retro",
];

/// Whether the player won the table round of outcome `n` (see `table_round`).
pub fn table_won(n: u32) -> bool {
    n & 4 == 0
}

/// Foosball (first to 10) or ping-pong (first to 11): a seeded quick round, then a quip.
pub fn table_round(def: &PropDef, n: u32) -> String {
    let (game, to) = if def.key.contains("pingpong") {
        ("ping-pong", 11)
    } else {
        ("foosball", 10)
    };
    let rival = pick(&RIVALS, n >> 3);
    let other = n % to;
    let (score, quip) = if table_won(n) {
        (format!("{to}-{other} to you"), pick(&TABLE_WIN, n >> 6))
    } else {
        (format!("{to}-{other} to them"), pick(&TABLE_LOSS, n >> 6))
    };
    format!("Quick round of {game} vs {rival}: {score}. {quip}")
}

const BEANBAG: [&str; 4] = [
    "You sink into the bean bag. It keeps sinking. For ten minutes nobody can find you.",
    "Power nap. You dream of a meeting that ended early. Doctors call it a lucid dream.",
    "The bean bag exhales as you sit. Same, bean bag. Same.",
    "You close your eyes 'to think about the architecture'. It works. You think about nothing.",
];

const QUIET: [&str; 4] = [
    "The Quiet Room. No screens, no calls, no AI. Just a fern and the hum of your thoughts.",
    "Someone left a note: 'Please do not book this room'. Below it, three people booked it.",
    "You sit in silence. For the first time this quarter, nobody needs a quick sync.",
    "The room is so quiet you can hear the open space deciding things without you.",
];

const VR: [&str; 4] = [
    "Headset on. Your legs are not rendered.",
    "Welcome to the metaverse. The meeting is also here.",
    "Investors loved this in 2021. Your controls are mirrored for 15 s.",
    "Please remove the headset. Someone needs the conference room.",
];

const PRINT_3D: [&str; 3] = [
    "PRINTING... 0%. The progress bar has been honest since the Q3 offsite.",
    "The printer hums and prints a tiny prototype of the word 'synergy'. It collapses.",
    "Estimated time remaining: after the roadmap. It has been 0% for so long it counts as stable.",
];

const DOG: [&str; 5] = [
    "Biscuit sniffs your badge and approves your access without a ticket. Best stakeholder here.",
    "Biscuit rolls over. You pet the belly. Your heart rate drops below 'incident'.",
    "Biscuit brings you a tennis ball covered in what you hope is drool. You throw it. Joy.",
    "Biscuit leans on your leg. No agenda, no follow-up, no action items.",
    "Biscuit has never once asked for an ETA. You feel seen.",
];

const BIKE: [&str; 4] = [
    "The bike has been here since the return-to-office memo. It does not judge you.",
    "A note on the saddle: 'Back after the reorg.' Dated 2019.",
    "The rack has room for twelve. The car park has room for four hundred.",
    "The bike's bell rings faintly in the wind. It sounds like hope, but rusty.",
];

/// While sitting on the smoking-corner bench.
pub const BENCH: [&str; 8] = [
    "You sit. The bench sighs with you.",
    "Somewhere, an agent is opening a ticket about your absence.",
    "A pigeon lands, looks at you, and leaves. Even it has meetings.",
    "From inside: 'Who approved the AI to approve things?' Nobody answers.",
    "Your phone buzzes: 'Quick sync?' You put it face down. Growth.",
    "The 'This is fine' sign is starting to look like a mission statement.",
    "A senior dev walks past, nods, and says nothing. Respect.",
    "For a moment, nothing is AI-powered. It is wonderful.",
];

const COFFEE_OK: [&str; 4] = [
    "The machine gurgles, thinks, and pours something brown. Fresh Brew: more mana in combat for 2 min.",
    "It brews a cup and asks for feedback on the cup. Fresh Brew for 2 min.",
    "Coffee! It is labelled 'AI-roasted'. It tastes like coffee anyway. Fresh Brew for 2 min.",
    "The machine recognises you and brews 'your usual'. You have never been here. Fresh Brew for 2 min.",
];

/// The local player's line for a prop outcome; `nth` counts this player's uses, so props
/// whose outcome repeats (a successful brew) still vary their line.
pub fn mine(def: &PropDef, outcome: u32, nth: u32, content: &Content) -> Option<String> {
    let n = outcome;
    let own_lines = !def.lines.is_empty()
        && !matches!(
            def.kind,
            PropKind::Vending
                | PropKind::Table
                | PropKind::Scooter
                | PropKind::FireAlarm
                | PropKind::ShuttleStop
        )
        && (def.kind != PropKind::Coffee || n != 0)
        // A device's own lines are for the step it took; refusals use the stock ones.
        && (def.kind != PropKind::Device || n == combat_core::QUEST_PROP_COUNTED);
    if own_lines {
        let k = n.wrapping_add(nth) as usize % def.lines.len();
        return def.lines.get(k).cloned();
    }
    Some(match def.kind {
        PropKind::Coffee if n == 0 => "Descaling required. Since 2019.".to_string(),
        PropKind::Coffee => pick(&COFFEE_OK, nth).to_string(),
        PropKind::Vending if n == 0 => format!(
            "The spiral turns. Nothing falls. The machine keeps your {} Kudos.",
            def.cost_sp
        ),
        PropKind::Vending => format!(
            "*clunk* Out drops: {}. ({} Kudos)",
            reward_name(def, n, content),
            def.cost_sp
        ),
        PropKind::Printer if def.key == "printer_3d" => pick(&PRINT_3D, nth).to_string(),
        PropKind::Printer => format!("The printer says: {}", pick(&PRINTER, n)),
        PropKind::VrDemo => pick(&VR, nth).to_string(),
        PropKind::Whiteboard => format!("The whiteboard suggests: {}", strategy(n)),
        PropKind::Billboard => format!("AI Billboard: \"{}\"", pick(&BILLBOARD, n)),
        PropKind::AiCar => format!("The AI-powered car: {}", pick(&CAR, n)),
        PropKind::Music => pick(&MUSIC, n).to_string(),
        PropKind::Table => table_round(def, n),
        PropKind::BeanBag => pick(&BEANBAG, nth).to_string(),
        PropKind::QuietRoom => pick(&QUIET, nth).to_string(),
        PropKind::Dog => pick(&DOG, nth).to_string(),
        PropKind::BikeRack => pick(&BIKE, n).to_string(),
        PropKind::Bench => BENCH[0].to_string(),
        PropKind::DaysSign => {
            "You reset the counter. Days without an AI incident: 0. It felt honest.".to_string()
        }
        PropKind::FireAlarm => format!(
            "*RIIIIING* {n} colleagues stop pretending to work and come for you. Nobody evacuates."
        ),
        PropKind::Scooter => {
            "Unlocked with your face. The AI recognised you as 'Scooter Owner'.".to_string()
        }
        PropKind::SprintBoard => {
            "The board shows this week's affixes. Someone already estimated your death at 3 points."
                .to_string()
        }
        PropKind::ScrumBoard => {
            "Your name is on three sticky notes. One of them just says 'why'.".to_string()
        }
        PropKind::Calendar => {
            "Every slot is double-booked. Yours is triple-booked, with yourself.".to_string()
        }
        PropKind::ShuttleStop if n == 0 => "The shuttle is not running. It says 'soon'.".into(),
        PropKind::ShuttleStop => format!(
            "The campus shuttle drives itself, badly. Next stop: {}.",
            content
                .prop(&def.to)
                .map_or("somewhere", |p| p.name.as_str())
        ),
        PropKind::Pickup => format!("You pocket the {}.", def.name),
        PropKind::Inspect => format!(
            "*click* {} photographed, timestamped, filed. Nobody will read it.",
            def.name
        ),
        PropKind::Device if n == combat_core::QUEST_PROP_WRONG_ORDER => format!(
            "{}: wrong order. Everything you did resets, as the runbook warned, on page 40.",
            def.name
        ),
        PropKind::Device if n == combat_core::QUEST_PROP_COUNTED => {
            format!("{}: done. It does not say thank you.", def.name)
        }
        PropKind::Device => format!(
            "The {} hums at you. Nothing to do here right now.",
            def.name
        ),
    })
}

/// What a reward pick of `outcome` (1-based, auras first) is called.
pub fn reward_name(def: &PropDef, outcome: u32, content: &Content) -> String {
    let k = outcome.saturating_sub(1) as usize;
    match def.auras.get(k) {
        Some((aura, _)) => content
            .aura(aura)
            .map_or_else(|| aura.clone(), |a| a.name.clone()),
        None => def
            .items
            .get(k - def.auras.len().min(k))
            .cloned()
            .unwrap_or_default(),
    }
}

/// A teammate's use, when it is worth a line in everyone's chat.
pub fn witnessed(def: &PropDef, name: &str) -> Option<String> {
    match def.kind {
        PropKind::FireAlarm => Some(format!("{name} pulled the fire alarm. Run. Or fight.")),
        PropKind::Scooter => Some(format!("{name} borrowed an e-scooter with their face.")),
        PropKind::AiCar => Some(format!(
            "Somewhere in the car park, {name} honks at the AI car."
        )),
        _ => None,
    }
}

/// The two-line tooltip under the prop's name: what a click does, and a warning or cost.
pub fn tooltip(def: &PropDef) -> (&'static str, String) {
    let action = match def.kind {
        PropKind::Coffee => "Brew a coffee",
        PropKind::Vending => "Buy a mystery snack",
        PropKind::Printer => "Print something (it is jammed)",
        PropKind::Whiteboard => "Generate a strategy",
        PropKind::Billboard => "Read the billboard",
        PropKind::Table => "Play a round",
        PropKind::Music => "Press the music button",
        PropKind::DaysSign => "Reset the counter",
        PropKind::FireAlarm => "Pull the fire alarm",
        PropKind::Bench => "Sit down (/sit)",
        PropKind::AiCar => "Honk",
        PropKind::BikeRack => "Keep the bike company",
        PropKind::Scooter => "Borrow (unlock with your face)",
        PropKind::SprintBoard => "Plan a sprint (timed run)",
        PropKind::ScrumBoard => "Daily Standup (daily and weekly tasks)",
        PropKind::Calendar => "Company Calendar (minigames and quest chains)",
        PropKind::BeanBag => "Take a power nap",
        PropKind::QuietRoom => "Hide in the quiet room",
        PropKind::Dog => "Pet the office dog",
        PropKind::VrDemo => "Put on a VR headset (15 s)",
        PropKind::ShuttleStop => "Ride the campus shuttle",
        PropKind::Pickup => "Pick up",
        PropKind::Inspect => "Photograph it for the report",
        PropKind::Device => "Operate",
    };
    let extra = match def.kind {
        PropKind::FireAlarm => "WARNING: every enemy on this floor will come for you.".to_string(),
        PropKind::Coffee if def.fail_pct > 0 => {
            format!("{}% chance: descaling required.", def.fail_pct)
        }
        PropKind::Vending => format!("Costs {} Kudos. No refunds.", def.cost_sp),
        PropKind::Scooter => "+60% speed out of combat. Damage ends the ride.".to_string(),
        PropKind::Printer => "Users report persistence helps. Users are wrong.".to_string(),
        PropKind::BikeRack => "It has been waiting a long time.".to_string(),
        PropKind::VrDemo => "Mirrored controls for 15 s. Not in combat.".to_string(),
        PropKind::ShuttleStop => "Fast travel to the next stop on the loop. Not in combat.".into(),
        PropKind::SprintBoard => {
            "Story points, affixes, a par time. Velocity is mandatory.".to_string()
        }
        PropKind::ScrumBoard => {
            "Three dailies a day, one Sprint Goal a week. Closes your Jira tickets for Kudos."
                .to_string()
        }
        PropKind::Calendar => {
            "Every side activity in one place. Closes your Jira tickets for Kudos.".to_string()
        }
        _ => String::new(),
    };
    let extra = match def.calm {
        Some((amount, cd_ms)) => format!(
            "{extra} Burnout -{}% (every {} min).",
            amount / 10,
            cd_ms.div_ceil(60_000)
        )
        .trim_start()
        .to_string(),
        None => extra,
    };
    (action, extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategies_read_like_a_deck_and_vary_with_the_roll() {
        let a = strategy(0);
        assert_eq!(
            a,
            "Q1 strategy: Leverage our AI-native synergies to unlock shareholder vibes."
        );
        assert_ne!(strategy(1), a);
        assert_ne!(strategy(1 << 9), a);
        assert!(strategy(u32::MAX).starts_with("Q4 strategy: Democratize"));
    }

    #[test]
    fn every_prop_of_the_shipped_content_has_lines_and_a_tooltip() {
        let c = match Content::from_ron(include_str!("../../../../content/game.ron")) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        for p in &c.world.props {
            for n in [0, 1, 2, 3, 65535] {
                let line = mine(p, n, 0, &c).unwrap_or_default();
                assert!(!line.is_empty(), "{} {n}", p.key);
            }
            assert!(!tooltip(p).0.is_empty());
        }
        // Zone-flavoured props speak their own lines; vending still names the reward.
        for key in [
            "gong_sales",
            "printer_board_deck",
            "whiteboard_hall_b",
            "coffee_exec",
        ] {
            let Some(p) = c.prop(key) else {
                panic!("no prop {key}");
            };
            for n in 1..6 {
                let line = mine(p, n, n, &c).unwrap_or_default();
                assert!(p.lines.contains(&line), "{key}: {line}");
            }
        }
        if let Some(p) = c.prop("coffee_exec") {
            assert_eq!(
                mine(p, 0, 3, &c).as_deref(),
                Some("Descaling required. Since 2019.")
            );
        }
        let vending = c.prop("vending_open_space");
        let names: Vec<String> = (1..=5)
            .map(|n| vending.map(|v| reward_name(v, n, &c)).unwrap_or_default())
            .collect();
        assert_eq!(names[0], "Sugar Rush");
        assert_eq!(names[4], "Energy Drink");
    }

    #[test]
    fn a_table_round_has_a_score_and_relief_props_say_so() {
        let c = match Content::from_ron(include_str!("../../../../content/game.ron")) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        };
        let Some(foosball) = c.prop("foosball_open_space") else {
            panic!("no foosball");
        };
        let line = table_round(foosball, 3);
        assert!(line.starts_with("Quick round of foosball vs "), "{line}");
        assert!(line.contains("10-3 to you"), "{line}");
        let pong = c.prop("pingpong_open_space").map(|p| table_round(p, 4 + 2));
        assert!(pong.is_some_and(|l| l.contains("ping-pong") && l.contains("11-6 to them")));
        assert!(tooltip(foosball).1.ends_with("Burnout -12% (every 3 min)."));
    }
}
