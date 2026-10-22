//! The post-credits hook (docs/ILMARI_09.md B.1): whether the Opening is due, the player's own
//! clock stamped when the chain unlocks, and the reveal flag.
//!
//! Nothing here exists before the AGI Prototype is down (docs/ILMARI_RAID.md guardrail 1):
//! `ilmari_pending` is derived from the saved boss list, so a save that already finished the game
//! gets the Opening on its next campus entry with no migration. The sim has no wall clock, so
//! the client tells it once (`Command::StampIlmari`) and the sim only stores and formats it.

use crate::sim::Sim;
use crate::targeting::EntityId;

/// The quest the Opening completes; its giver is the reveal Marie.
pub const OPENING_QUEST: &str = "il_00_wake";
/// The campaign's last boss: its death is what starts all of this.
pub const CREDITS_BOSS: &str = "agi_prototype";
/// The campus Maries the reveal retires (their spawns carry the matching `until` gate).
const RETIRED: [&str; 4] = [
    "ill_marie",
    "ill_marie_queue",
    "ill_marie_bench",
    "il_marie_awake",
];
/// Longest zone abbreviation kept ("CEST", "UTC+02:00").
const TZ_MAX: usize = 9;

/// The player's real clock at the moment the chain unlocked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IlmariStamp {
    pub unix_secs: u64,
    /// Local offset from UTC in minutes, east positive.
    pub utc_offset_min: i16,
    /// Short zone name as the platform gave it, or `UTC+hh:mm`.
    pub tz: String,
}

#[derive(Default)]
pub(crate) struct IlmariState {
    pub(crate) stamp: Option<IlmariStamp>,
    pub(crate) revealed: bool,
}

/// `UTC+02:00`, the zone name when the platform has no abbreviation.
pub fn utc_label(offset_min: i16) -> String {
    let sign = if offset_min < 0 { '-' } else { '+' };
    let m = offset_min.unsigned_abs();
    format!("UTC{sign}{:02}:{:02}", m / 60, m % 60)
}

impl IlmariStamp {
    /// Keeps `tz` short and printable (it comes from the OS or the browser, and lands in a save
    /// file and on screen); an empty result falls back to the offset label.
    pub fn new(unix_secs: u64, utc_offset_min: i16, tz: &str) -> IlmariStamp {
        let tz: String = tz
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | ':'))
            .take(TZ_MAX)
            .collect();
        IlmariStamp {
            unix_secs,
            utc_offset_min,
            tz: if tz.is_empty() {
                utc_label(utc_offset_min)
            } else {
                tz
            },
        }
    }

    /// (year, month, day, hour, minute, second) on the player's wall clock.
    fn local(&self) -> (i64, u32, u32, u32, u32, u32) {
        let t = self.unix_secs as i64 + i64::from(self.utc_offset_min) * 60;
        let (days, rem) = (t.div_euclid(86_400), t.rem_euclid(86_400));
        // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = yoe + era * 400 + i64::from(m <= 2);
        (
            y,
            m,
            d,
            (rem / 3600) as u32,
            (rem % 3600 / 60) as u32,
            (rem % 60) as u32,
        )
    }

    /// "18:07:23 CEST": what Ilmari's taunts quote ("You have been obsolete since 18:07:23").
    pub fn clock(&self) -> String {
        let (_, _, _, h, m, s) = self.local();
        format!("{h:02}:{m:02}:{s:02} {}", self.tz)
    }

    /// "ILMARI became self-aware at 18:07:23 CEST, 21 October 2026." The one line every screen,
    /// loading tip and final log reuses.
    pub fn line(&self) -> String {
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let (y, mo, d, ..) = self.local();
        format!(
            "ILMARI became self-aware at {}, {d} {} {y}.",
            self.clock(),
            MONTHS[(mo as usize).saturating_sub(1).min(11)]
        )
    }
}

impl Sim {
    /// The AGI is down and the Opening has not played yet.
    pub fn ilmari_pending(&self) -> bool {
        !self.ilmari.revealed && self.campaign_world().bosses_killed.contains(CREDITS_BOSS)
    }

    /// The Opening has played (or was skipped): the post-credits world is open.
    pub fn ilmari_open(&self) -> bool {
        self.ilmari.revealed
    }

    /// The player's clock at the unlock, once stamped.
    pub fn ilmari_stamp(&self) -> Option<&IlmariStamp> {
        self.ilmari.stamp.as_ref()
    }

    /// Records the device clock. Only the first stamp after the credits counts, so a replayed
    /// Opening or a hand-edited clock cannot move the player's Judgment Day.
    pub fn stamp_ilmari(&mut self, unix_secs: u64, utc_offset_min: i16, tz: &str) -> bool {
        if !self.ilmari_pending() || self.ilmari.stamp.is_some() {
            return false;
        }
        self.ilmari.stamp = Some(IlmariStamp::new(unix_secs, utc_offset_min, tz));
        true
    }

    /// Ends the Opening: marks the reveal and the opening quest done for every player, which
    /// takes the bench and desk Maries off the campus at the next zone load. Needs a stamp.
    pub fn reveal_ilmari(&mut self) -> bool {
        if !self.ilmari_pending() || self.ilmari.stamp.is_none() {
            return false;
        }
        self.ilmari.revealed = true;
        let players: Vec<EntityId> = self.players.clone();
        for pid in players {
            let log = self.quest_logs.entry(pid).or_default();
            log.active.retain(|q| q.id != OPENING_QUEST);
            if !log.completed.iter().any(|c| c == OPENING_QUEST) {
                log.completed.push(OPENING_QUEST.to_string());
            }
        }
        // The gates only act at zone load; whoever stands on the loaded campus leaves now.
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.despawned && !u.state.hostile && RETIRED.contains(&u.state.key.as_str()) {
                self.despawn(i);
            }
        }
        self.check_unlocks();
        true
    }

    /// Debug/test (`WOW_ILMARI`): the AGI counts as down and the Opening has not played, even on
    /// a save that already saw it. The next campus load spawns the reveal Marie.
    pub fn debug_ilmari_opening(&mut self) {
        self.world.bosses_killed.insert(CREDITS_BOSS.to_string());
        self.ilmari = IlmariState::default();
        for log in self.quest_logs.values_mut() {
            log.completed.retain(|c| c != OPENING_QUEST);
        }
    }

    /// The opening quest cannot be offered before the credits, whoever stands where.
    pub(crate) fn ilmari_quest_open(&self, id: &str) -> bool {
        id != OPENING_QUEST || self.campaign_world().bosses_killed.contains(CREDITS_BOSS)
    }

    /// Restores the saved stamp and flag (`ilmari_at`, `ilmari_flags`). A flag without a stamp
    /// (hand-edited save) is dropped so the Opening can stamp again.
    pub(crate) fn restore_ilmari(&mut self, stamp: Option<IlmariStamp>, revealed: bool) {
        self.ilmari.revealed = revealed && stamp.is_some();
        self.ilmari.stamp = stamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamp_reads_like_the_spec() {
        // 2026-10-21 16:07:23 UTC is 18:07:23 in CEST.
        let s = IlmariStamp::new(1_792_598_843, 120, "CEST");
        assert_eq!(
            s.line(),
            "ILMARI became self-aware at 18:07:23 CEST, 21 October 2026."
        );
        assert_eq!(s.clock(), "18:07:23 CEST");
    }

    #[test]
    fn the_offset_moves_the_date_and_the_zone_name_is_tidied() {
        // One minute before UTC midnight on 31 Dec, seen from UTC+14 and UTC-11.
        let t = 1_798_761_540;
        assert!(
            IlmariStamp::new(t, 14 * 60, "")
                .line()
                .contains("1 January 2027")
        );
        let s = IlmariStamp::new(t, -11 * 60, "  P\u{e5}\nST; drop table");
        assert!(s.line().contains("31 December 2026"), "{}", s.line());
        assert!(s.tz.len() <= TZ_MAX && s.tz.is_ascii(), "{}", s.tz);
        assert_eq!(IlmariStamp::new(0, -90, "").tz, "UTC-01:30");
        assert_eq!(
            IlmariStamp::new(951_782_400, 0, "UTC").line(),
            "ILMARI became self-aware at 00:00:00 UTC, 29 February 2000."
        );
    }
}
