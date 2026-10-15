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

/// One per transition, in order.
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
