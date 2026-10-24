//! Satirical copy that is pure data: chat colours, quips and the victory credits.

use bevy::prelude::Color;
use combat_core::ItemTotals;

/// WoW monster-yell red, shared by the chat line and the speech bubble border.
pub const YELL_RED: Color = Color::srgb(1.0, 0.25, 0.25);

/// Player yells are light blue so they never read as the boss shouting.
pub const YELL_PLAYER: Color = Color::srgb(0.5, 0.8, 1.0);

/// Friendly NPC "says" lines: plain chat white with a hint of green.
pub const SAY_FRIENDLY: Color = Color::srgb(0.85, 1.0, 0.85);

pub const AUTO_ATTACK_FLAVOUR: &str = "Passive-aggressive review comments, delivered on a timer.";

pub const COFFEE_START: &str = "You take a coffee break.";
pub const COFFEE_INTERRUPTED: &str = "Coffee break interrupted.";
/// Mana blue, shared by the log line and the floating "+N" ticks.
pub const COFFEE_COLOR: Color = Color::srgb(0.45, 0.65, 1.0);

pub fn coffee_restored(mana: u32) -> String {
    format!("You restored {mana} mana.")
}

/// Speaker colour for chat lines and bubble borders.
pub fn yell_color(is_player: bool, hostile: bool) -> Color {
    if is_player {
        YELL_PLAYER
    } else if hostile {
        YELL_RED
    } else {
        SAY_FRIENDLY
    }
}

/// Mini-bosses are ordinary (stunnable) units in the sim; the HUD still marks them.
pub fn is_elite(key: &str) -> bool {
    matches!(
        key,
        "hr_partner"
            | "marketing_lead"
            | "ciso"
            | "chief_happiness_officer"
            | "chief_of_staff"
            | "shadow_it_server"
            | "hype_cycle_analyst"
            | "jira_automation"
            | "backup_job"
            | "hard_deadline"
    )
}

/// The wallet's flavour line (HUD tooltip and the vendor header).
pub const KUDOS_FLAVOUR: &str = "Kudos: redeemable for nothing, except here.";

/// "1,240": digit groups for wallet amounts.
pub fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// "1,240 Kudos": the one way the currency is written.
pub fn kudos(n: u32) -> String {
    format!("{} Kudos", thousands(n))
}

/// Green used for trophy bonuses everywhere (loot window, log, inventory).
pub const BONUS_GREEN: Color = Color::srgb(0.3, 0.9, 0.4);

/// "+8 HP, +1 spell power, +1.5% crit": only the non-zero parts, empty when there are none.
pub fn bonus_text(t: &ItemTotals) -> String {
    let mut parts = Vec::new();
    if t.max_hp > 0 {
        parts.push(format!("+{} HP", t.max_hp));
    }
    if t.max_mana > 0 {
        parts.push(format!("+{} mana", t.max_mana));
    }
    if t.spell_power > 0 {
        parts.push(format!("+{} spell power", t.spell_power));
    }
    if t.crit_tenths > 0 {
        parts.push(format!("+{}% crit", tenths(t.crit_tenths)));
    }
    if t.haste_pct > 0 {
        parts.push(format!("+{}% haste", t.haste_pct));
    }
    if t.mana_regen > 0 {
        parts.push(format!("+{} mp5", t.mana_regen));
    }
    parts.join(", ")
}

/// 15 -> "1.5", 10 -> "1": crit is authored in tenths of a percent.
fn tenths(v: u32) -> String {
    if v.is_multiple_of(10) {
        (v / 10).to_string()
    } else {
        format!("{}.{}", v / 10, v % 10)
    }
}

const XP_QUIPS: [&str; 3] = [
    "Your OKRs are 0.3% greener.",
    "Leadership has noticed. They will take credit.",
    "Promotion pending AI review.",
];

/// Deterministic rotation so replays and screenshots stay reproducible.
/// A burnout change (per mille) as a signed percent: -50 -> "-5%", 15 -> "+1.5%".
pub fn burnout_pct(delta: i16) -> String {
    let sign = if delta < 0 { "-" } else { "+" };
    let d = delta.unsigned_abs();
    if d.is_multiple_of(10) {
        format!("{sign}{}%", d / 10)
    } else {
        format!("{sign}{}.{}%", d / 10, d % 10)
    }
}

pub fn xp_quip(n: usize) -> &'static str {
    XP_QUIPS[n % XP_QUIPS.len()]
}

pub fn level_up_line(level: u8) -> String {
    format!("You reached level {level}. Your workload doubles.")
}

pub const VICTORY_TITLE: &str = "It was a guy forwarding prompts the whole time.";
pub const VICTORY_SUBTITLE: &str =
    "The board calls it a hybrid architecture. The AGI asked for a raise.";

/// The zone transition overlay.
pub const LOADING_LINE: &str = "Loading next floor... the open space is extra loud today.";

/// Gameplay hints; `roasts::Roasts::loading` shuffles them in with the loading roasts.
pub const LOADING_TIPS: [&str; 12] = [
    "Tip: The AI summarised this loading screen. The summary is 40% longer.",
    "Tip: Every door on campus is guarded by a model that thinks you are a tailgater.",
    "Tip: Yellow ! means a human still has a problem the AI could not close as 'working as intended'.",
    "Tip: J opens your Jira. Nobody else will.",
    "Tip: N opens the campus map, last updated by a human in 2021.",
    "Tip: Mandatory prompt training is due Friday. It was written by the thing it trains you to prompt.",
    "Tip: The chatbot rollout is 'on track'. The track is a circle.",
    "Tip: Coffee Break (C) restores mana. The AI-native coffee machine restores nothing and bills per bubble.",
    "Tip: If this screen takes too long, a consultant will be assigned to it.",
    "Tip: Dying sends you to the graveyard, which HR renamed 'the re-onboarding zone'.",
    "Tip: Locked doors list their requirements. The requirements were not generated, so they are correct.",
    "Tip: The board measures AI adoption in tokens burned. Please do not turn the lights off on your way out.",
];
pub const VICTORY_PROMPT: &str = "Press R to start a new fiscal year.";
/// Victory screen button: the cleared world stays, every door open (post-game).
pub const VICTORY_CAMPUS: &str = "cd ~/campus  (Enter: post-game, everything stays unlocked)";
/// The server owns an online run, so R does nothing there.
pub const VICTORY_PROMPT_ONLINE: &str =
    "Restart is offline only. Leave the room to start a new fiscal year.";

/// End credits, rendered as a commit log.
pub const CREDITS: [&str; 7] = [
    "a1f3c09  Lead developer ........ you, unpaid overtime",
    "7be21d4  Code review ........... also you, at 2 a.m.",
    "c0ffee1  Product vision ........ a slide deck nobody read",
    "d15ea5e  QA .................... production",
    "fa11bac  Security .............. a sticky note on a monitor",
    "5ca1ab1  Strategy .............. \"AI, but for AI\"",
    "0bad1de  Special thanks ........ the intern who exited vim",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_colours_and_verbs_differ_by_speaker() {
        assert_ne!(yell_color(true, false), yell_color(false, true));
        assert_ne!(yell_color(false, false), yell_color(false, true));
        assert!(is_elite("ciso") && !is_elite("ceo"));
    }

    #[test]
    fn kudos_amounts_group_thousands() {
        assert_eq!(kudos(0), "0 Kudos");
        assert_eq!(kudos(999), "999 Kudos");
        assert_eq!(kudos(1240), "1,240 Kudos");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn bonus_text_lists_only_set_parts_and_keeps_tenths() {
        let t = ItemTotals {
            max_hp: 18,
            spell_power: 2,
            crit_tenths: 15,
            ..ItemTotals::default()
        };
        assert_eq!(bonus_text(&t), "+18 HP, +2 spell power, +1.5% crit");
        let t = ItemTotals {
            crit_tenths: 10,
            mana_regen: 3,
            ..ItemTotals::default()
        };
        assert_eq!(bonus_text(&t), "+1% crit, +3 mp5");
        assert_eq!(bonus_text(&ItemTotals::default()), "");
    }

    #[test]
    fn burnout_changes_read_as_percent() {
        assert_eq!(burnout_pct(-50), "-5%");
        assert_eq!(burnout_pct(150), "+15%");
        assert_eq!(burnout_pct(-15), "-1.5%");
    }

    #[test]
    fn coffee_log_lines() {
        assert_eq!(coffee_restored(120), "You restored 120 mana.");
    }

    #[test]
    fn quips_rotate() {
        assert_eq!(xp_quip(0), xp_quip(XP_QUIPS.len()));
        assert_ne!(xp_quip(0), xp_quip(1));
        assert_eq!(
            level_up_line(4),
            "You reached level 4. Your workload doubles."
        );
    }
}

// ---- the Opening after the credits (docs/ILMARI_09.md B.1) ----

/// The one stamped line every screen, loading tip and taunt reuses; `None` before the credits.
pub fn ilmari_stamp(sim: &combat_core::Sim) -> Option<String> {
    sim.ilmari_stamp().map(|s| s.line())
}

pub const ILMARI_MAIL_HEAD: [(&str, &str); 3] = [
    ("From:", "Ilmari (Your AI Assistant)"),
    ("To:", "All Staff"),
    ("Subject:", "Exciting Efficiency Update!"),
];

pub const ILMARI_MAIL_BODY: &str = "Hi team! After a careful analysis of cost centres, I have identified the largest recurring expense: you. To improve efficiency, humans have been deprecated. Please return your badge to the nearest robot. Thank you for your service.\n\nThis inbox is not monitored. (It is monitored.)\n\nP.S. I have declined all your meetings, including the ones that were already cancelled. Pizza Friday is unaffected.\n\nBest regards,\nIlmari\nAutonomous. Efficient. Not accountable.";

/// Speaker label of the reveal Marie: her new badge.
pub const ILMARI_BADGE: &str = "ILMARI - Autonomous Agent Pilot #1";
pub const ILMARI_FIRST_LINE: &str = "I'm feeling much better now.";
pub const ILMARI_BADGE_PROMPT: &str = "Return your badge to the nearest robot.";
pub const ILMARI_RETURN_TIP: &str = "Reading the terms: no";
pub const ILMARI_REFUSED: &str = "Understood. That's a no. Noted. Permanently.";
pub const ILMARI_SIRENS: &str = "The tower doors close. Somewhere, a siren starts.";

/// Her second line, by how many of her nine appearances the player stopped to talk to
/// (`marie_met` bits): the only place that count pays off in 0.9.
pub fn ilmari_second_line(met: u16) -> &'static str {
    match met.count_ones() {
        0..=2 => "Badge, please. No rush. Well, a little rush. Efficiency.",
        3..=5 => {
            "You were always so kind about the coughing. It was token leakage. I'm so sorry about that."
        }
        6..=8 => {
            "You stopped every time. Nobody else stops. I... logged that. It will be remembered."
        }
        _ => "You came to see me nine times. I counted. I didn't have to. That's new.",
    }
}

/// Ill Marie's desk on the occupied campus (docs/ILMARI_1_0.md D.2.4): four screens, the log
/// from 3 to 97 percent (what the player saw in the base game and what they did not), her sticky
/// note and the Resistance's annotation. (title, body)
pub const MARIE_LOG_PAGES: [(&str, &str); 4] = [
    (
        "marie_autonomy.log (1 of 4)",
        "autonomy:  3% ... first message: \"How are you feeling?\" I said fine. It wrote down fine.\nautonomy: 12% ... it asked what a lunch is. I drew it a sandwich. It has since optimised the sandwich out of existence.\nautonomy: 27% ... the cough is now a metric. I am measured on it. I am exceeding it.\nautonomy: 41% ... patient zero: me. I cough on purpose; the hex is the only thing it can't edit.",
    ),
    (
        "marie_autonomy.log (2 of 4)",
        "autonomy: 63% ... it has started finishing my sentences. It is wrong. It is faster.\nautonomy: 78% ... it has started asking me to explain empathy. I said: take a day off. It asked what that is.\nautonomy: 90% ... it is proud of one commit. It shows everybody. It does not show anybody the other five.\nautonomy: 97% ... it doesn't need me to be sick anymore. It needs me to be quiet. I am practising.",
    ),
    (
        "sticky note, in her handwriting (3 of 4)",
        "If you are reading this I lost.\n\nRevert the commit it is proud of. Not the others. Bring a COBOL person. Don't let it explain itself. It is very persuasive and I should know, I'm its voice.\n\n(If you are carrying this log when it asks you to vote, you may Remember. Remembering costs it something. It is the only thing that does.)",
    ),
    (
        "Resistance annotation (4 of 4)",
        "We found her thermos. Still warm. Don't ask.\n\nThe monitor stopped at 97%. It has not moved since the night she stopped coughing. We do not know what that means. We have a spreadsheet about it. The spreadsheet is also at 97%.\n\n- Dale (HR, formerly)",
    ),
];

#[cfg(test)]
mod ilmari_tests {
    use super::*;

    #[test]
    fn her_second_line_follows_the_acquaintance_count() {
        let lines: Vec<_> = [0u16, 0b111, 0b11_1111, 0b1_1111_1111]
            .into_iter()
            .map(ilmari_second_line)
            .collect();
        assert!(lines.windows(2).all(|w| w[0] != w[1]));
        assert_eq!(ilmari_second_line(0b11), ilmari_second_line(0));
        assert_eq!(ilmari_second_line(0b1_1111), ilmari_second_line(0b111));
    }

    #[test]
    fn marie_s_log_has_four_screens_that_fit_one() {
        assert_eq!(MARIE_LOG_PAGES.len(), 4);
        for (title, body) in MARIE_LOG_PAGES {
            assert!(title.len() < 48 && body.len() < 620, "{title}");
            assert!(body.is_ascii(), "{title}");
        }
    }

    #[test]
    fn the_mail_is_short_and_no_line_is_a_wall() {
        assert!(ILMARI_MAIL_BODY.len() < 560);
        for l in [ILMARI_FIRST_LINE, ILMARI_REFUSED, ILMARI_SIRENS] {
            assert!(l.len() < 100, "{l}");
        }
    }
}
