//! Satirical copy that is pure data: chat colours, quips and the victory credits.

use bevy::prelude::Color;

/// WoW monster-yell red, shared by the chat line and the speech bubble border.
pub const YELL_RED: Color = Color::srgb(1.0, 0.25, 0.25);

/// Player yells are light blue so they never read as the boss shouting.
pub const YELL_PLAYER: Color = Color::srgb(0.5, 0.8, 1.0);

/// Friendly NPC "says" lines: plain chat white with a hint of green.
pub const SAY_FRIENDLY: Color = Color::srgb(0.85, 1.0, 0.85);

pub const AUTO_ATTACK_FLAVOUR: &str = "Passive-aggressive review comments, delivered on a timer.";

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

/// Hostiles yell, everyone else talks.
pub fn speech_verb(hostile: bool) -> &'static str {
    if hostile { "yells" } else { "says" }
}

/// Mini-bosses are ordinary (stunnable) units in the sim; the HUD still marks them.
pub fn is_elite(key: &str) -> bool {
    matches!(key, "hr_partner" | "marketing_lead" | "ciso")
}

const XP_QUIPS: [&str; 3] = [
    "Your OKRs are 0.3% greener.",
    "Leadership has noticed. They will take credit.",
    "Promotion pending AI review.",
];

/// Deterministic rotation so replays and screenshots stay reproducible.
pub fn xp_quip(n: usize) -> &'static str {
    XP_QUIPS[n % XP_QUIPS.len()]
}

pub fn level_up_line(level: u8) -> String {
    format!("You reached level {level}. Your workload doubles.")
}

pub const VICTORY_TITLE: &str = "You defeated the CEO.";
pub const VICTORY_SUBTITLE: &str = "The board replaced him with an AI.";
pub const VICTORY_PROMPT: &str = "Press R to start a new sprint.";

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
        assert_eq!(speech_verb(true), "yells");
        assert_eq!(speech_verb(false), "says");
        assert!(is_elite("ciso") && !is_elite("ceo"));
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
