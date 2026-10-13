//! Satirical copy that is pure data: unit titles and the XP-gain quips.

use bevy::prelude::Color;

/// WoW monster-yell red, shared by the chat line and the speech bubble border.
pub const YELL_RED: Color = Color::srgb(1.0, 0.25, 0.25);

/// Player yells are light blue so they never read as the boss shouting.
pub const YELL_PLAYER: Color = Color::srgb(0.5, 0.8, 1.0);

pub const AUTO_ATTACK_FLAVOUR: &str = "Passive-aggressive review comments, delivered on a timer.";

pub fn yell_color(is_player: bool) -> Color {
    if is_player { YELL_PLAYER } else { YELL_RED }
}

/// Units that get the "PROD" tag on their frame.
pub fn is_boss(key: &str) -> bool {
    key == "ceo"
}

const XP_QUIPS: [&str; 3] = [
    "Your OKRs are 0.3% greener.",
    "Leadership has noticed. They will take credit.",
    "Promotion pending AI review.",
];

/// WoW-style "<Title>" under a unit name, keyed on the sim unit key (not the display name).
pub fn title_for(key: &str) -> &'static str {
    match key {
        "ceo" => "<Chief AI Visionary>",
        "intern" => "<Prompt Engineer>",
        "senior_engineer" => "<Legacy Cost Center>",
        _ => "",
    }
}

/// Deterministic rotation so replays and screenshots stay reproducible.
pub fn xp_quip(n: usize) -> &'static str {
    XP_QUIPS[n % XP_QUIPS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_follow_sim_keys() {
        assert_eq!(title_for("ceo"), "<Chief AI Visionary>");
        assert_eq!(title_for("intern"), "<Prompt Engineer>");
        assert_eq!(title_for("unknown"), "");
    }

    #[test]
    fn yell_colours_differ_by_speaker() {
        assert_ne!(yell_color(true), yell_color(false));
        assert!(is_boss("ceo") && !is_boss("intern"));
    }

    #[test]
    fn quips_rotate() {
        assert_eq!(xp_quip(0), xp_quip(XP_QUIPS.len()));
        assert_ne!(xp_quip(0), xp_quip(1));
    }
}
