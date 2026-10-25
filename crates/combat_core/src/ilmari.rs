//! The post-credits hook (docs/ILMARI_09.md B.1): whether the Opening is due, the player's own
//! clock stamped when the chain unlocks, and the reveal flag.
//!
//! Nothing here exists before the AGI Prototype is down (docs/ILMARI_RAID.md guardrail 1):
//! `ilmari_pending` is derived from the saved boss list, so a save that already finished the game
//! gets the Opening on its next campus entry with no migration. The sim has no wall clock, so
//! the client tells it once (`Command::StampIlmari`) and the sim only stores and formats it.

use crate::content::{ItemKind, TOKEN_STACK_CAP};
use crate::economy::GearSource;
use crate::live::week_index;
use crate::sim::Sim;
use crate::targeting::EntityId;
use serde::{Deserialize, Serialize};

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
/// The currency of the Resistance quartermaster (an `ItemKind::Token` of `Content::items`).
pub const TOKEN: &str = "Ilmari Token";
/// Tokens that can be earned per ISO week, so the shop is a pace and not a treadmill (B.2).
pub const TOKEN_WEEK_CAP: u32 = 200;
/// The occupied campus and the Resistance base: the two phases outside the wings.
pub const OCCUPIED_ZONE: &str = "campus_occupied";
pub const BASE_ZONE: &str = "resistance/1";

/// How hard an Ilmari encounter bites. Story is always on offer, any group size, solo included;
/// Full is the authored unit at the Ilmari item band (docs/ILMARI_09.md B.3); Judgment Day is
/// Full plus the `JudgmentDef` numbers and riders for a quorum of humans (docs/ILMARI_1_0.md D.1).
/// The wings read this to scale their units and key their lockouts; the open-world phase has a
/// single difficulty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IlmariMode {
    Story,
    #[default]
    Full,
    Judgment,
}

/// The name 0.9 and 0.10 gave the Story / Full pair.
pub type IlmariDifficulty = IlmariMode;

impl IlmariMode {
    /// (HP %, damage %) of the authored unit: Story reuses the raid's Async figures, Judgment
    /// sits on top of Full.
    pub fn scale(
        self,
        raid: &crate::raid::RaidDef,
        jd: &crate::judgment::JudgmentDef,
    ) -> (u32, u32) {
        match self {
            IlmariMode::Story => (raid.async_hp_pct, raid.async_damage_pct),
            IlmariMode::Full => (100, 100),
            IlmariMode::Judgment => (jd.hp_pct, jd.damage_pct),
        }
    }

    /// Prefix of the weekly lockout key (every mode locks separately).
    pub fn lock_prefix(self) -> &'static str {
        match self {
            IlmariMode::Story => crate::raid::STORY_PREFIX,
            IlmariMode::Full => "",
            IlmariMode::Judgment => crate::raid::JUDGMENT_PREFIX,
        }
    }
}

/// Everything Ilmari 1.0 adds to the content (all optional: minimal test content has none).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct IlmariDef {
    #[serde(default)]
    pub judgment: crate::judgment::JudgmentDef,
    #[serde(default)]
    pub rewards: crate::rewards::RewardsDef,
    #[serde(default)]
    pub ladder: crate::ladder::LadderDef,
}

/// Whether `zone` is Ilmari content: Ilmari gear bonuses work here and nowhere else.
pub fn is_ilmari_zone(zone: &str) -> bool {
    zone == OCCUPIED_ZONE || zone == BASE_ZONE || zone.starts_with("ilmari")
}

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
    /// Per player: tokens earned in ISO week `.0`; the weekly cap counts against `.1`. One
    /// entry offline, one per member in a co-op room so the cap is not shared by the group.
    pub(crate) tokens: std::collections::BTreeMap<EntityId, (u32, u32)>,
    /// Whether the worn Ilmari pieces were last summed as in Ilmari content.
    worn_in: bool,
    /// Tick of the first stamp or reveal from a non-lead member; after `LEAD_GRACE_MS` any
    /// member may finish the Opening, so an AFK lead cannot block the room.
    pub(crate) asked_at: Option<u64>,
    /// The room's sim has killed Ilmari on Full at least once: the key to Judgment Day. The core
    /// sets it (`mark_core_full_clear`); a save carries it, online it is the room's own progress.
    pub(crate) core_full: bool,
    /// The clock stamp of the core's fall (`ilmari_end`), once told.
    pub(crate) end: Option<crate::ilmari_core::IlmariEnd>,
    /// Judgment Day: players who spent their one revive in the pull that is running.
    pub(crate) jd_spent: std::collections::BTreeSet<EntityId>,
    /// The weekly boards of the core fight on Full and Judgment Day.
    pub(crate) ladders: crate::ladder::IlmariLadders,
}

/// How long the room waits for its lead to stamp and reveal before any member may.
pub(crate) const LEAD_GRACE_MS: u32 = 120_000;

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
        format!("ILMARI became self-aware at {}.", self.when())
    }

    /// "18:07:23 CEST, 21 October 2026": the clock and the date, for lines that carry their own verb.
    pub fn when(&self) -> String {
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
            "{}, {d} {} {y}",
            self.clock(),
            MONTHS[(mo as usize).saturating_sub(1).min(11)]
        )
    }
}

impl Sim {
    /// Stamp and reveal come from the room lead (the stamp is the room's Judgment Day and the
    /// reveal ends everyone's Opening); others only after the lead had `LEAD_GRACE_MS` to do it.
    pub(crate) fn ilmari_actor_ok(&mut self, actor: EntityId) -> bool {
        if self.players.first() == Some(&actor) {
            return true;
        }
        let since = *self.ilmari.asked_at.get_or_insert(self.now);
        self.now >= since + crate::content::ticks(LEAD_GRACE_MS)
    }

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

    /// A player joining a room whose Opening has played is past it too: the reveal marked the
    /// members of that moment, and the chain after it needs the quest in the joiner's own log.
    pub(crate) fn ilmari_on_join(&mut self, pid: EntityId) {
        if self.ilmari.revealed {
            let log = self.quest_logs.entry(pid).or_default();
            if !log.completed.iter().any(|c| c == OPENING_QUEST) {
                log.completed.push(OPENING_QUEST.to_string());
            }
        }
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
        // The ending's marker is written by `Command::EndingSeen` and never offered.
        if id == crate::rewards::ENDING_QUEST {
            return false;
        }
        id != OPENING_QUEST || self.campaign_world().bosses_killed.contains(CREDITS_BOSS)
    }

    /// The loaded zone is Ilmari content (`is_ilmari_zone`).
    pub fn in_ilmari_content(&self) -> bool {
        self.zone.as_deref().is_some_and(is_ilmari_zone)
    }

    /// Re-sums the gear of players who wear an Ilmari piece when the zone crossed the border of
    /// Ilmari content, keeping their HP and mana fractions like any gear change. Players
    /// without such a piece (all of the base game) are never touched.
    pub(crate) fn sync_ilmari_gear(&mut self) {
        let now = self.in_ilmari_content();
        if now == self.ilmari.worn_in {
            return;
        }
        self.ilmari.worn_in = now;
        for pid in self.players.clone() {
            let Some(i) = self.index(pid) else { continue };
            let wears = self.units[i]
                .state
                .equipped
                .iter()
                .filter_map(|n| self.economy().gear(n))
                .any(|g| g.source == GearSource::Ilmari);
            if !wears {
                continue;
            }
            let (old_hp, old_mana) = (self.units[i].state.max_hp, self.units[i].state.max_mana);
            self.refresh_item_bonus(i);
            let s = &mut self.units[i].state;
            let keep = |cur: u32, old: u32, new: u32| {
                if old == 0 {
                    cur.min(new)
                } else {
                    ((u64::from(cur) * u64::from(new) / u64::from(old)) as u32).clamp(1, new.max(1))
                }
            };
            if s.alive {
                s.hp = keep(s.hp, old_hp, s.max_hp);
            }
            s.mana = keep(s.mana, old_mana, s.max_mana).min(s.max_mana);
        }
    }

    /// Copies of the stack `name` in player `pi`'s inventory.
    pub(crate) fn token_count(&self, pi: usize, name: &str) -> u32 {
        self.units[pi]
            .state
            .inventory
            .iter()
            .find(|(n, _)| n == name)
            .map_or(0, |(_, c)| *c)
    }

    /// Takes `n` tokens (callers checked `token_count`); an emptied stack leaves the inventory.
    pub(crate) fn spend_tokens(&mut self, pi: usize, name: &str, n: u32) {
        let inv = &mut self.units[pi].state.inventory;
        if let Some(k) = inv.iter().position(|(m, _)| m == name) {
            inv[k].1 = inv[k].1.saturating_sub(n);
            if inv[k].1 == 0 {
                inv.remove(k);
            }
        }
        self.bag_event(pi);
    }

    /// Tokens still earnable this ISO week (`TOKEN_WEEK_CAP`) by the first player.
    pub fn ilmari_tokens_left(&self) -> u32 {
        self.players
            .first()
            .map_or(TOKEN_WEEK_CAP, |p| self.ilmari_tokens_left_of(*p))
    }

    /// Same for any player of the room.
    pub fn ilmari_tokens_left_of(&self, pid: EntityId) -> u32 {
        let week = week_index(self.unix_s());
        let (w, earned) = self.ilmari.tokens.get(&pid).copied().unwrap_or_default();
        TOKEN_WEEK_CAP.saturating_sub(if w == week { earned } else { 0 })
    }

    /// Pays `n` tokens (a quest, the daily) within the weekly cap and the stack cap; returns what
    /// was actually granted. Offline there is one wallet, so the week's counter is the sim's.
    pub fn grant_ilmari_tokens(&mut self, pid: EntityId, n: u32) -> u32 {
        let Some(pi) = self.index(pid) else { return 0 };
        let is_token = self
            .content
            .item(TOKEN)
            .is_some_and(|i| i.kind == ItemKind::Token);
        if !is_token {
            return 0;
        }
        let week = week_index(self.unix_s());
        let slot = self.ilmari.tokens.entry(pid).or_default();
        if slot.0 != week {
            *slot = (week, 0);
        }
        let earned = slot.1;
        let held = self.token_count(pi, TOKEN);
        let n = n
            .min(TOKEN_WEEK_CAP - earned.min(TOKEN_WEEK_CAP))
            .min(TOKEN_STACK_CAP.saturating_sub(held));
        if n == 0 {
            return 0;
        }
        if let Some(slot) = self.ilmari.tokens.get_mut(&pid) {
            slot.1 += n;
        }
        let inv = &mut self.units[pi].state.inventory;
        match inv.iter_mut().find(|(m, _)| m == TOKEN) {
            Some((_, c)) => *c += n,
            None => inv.push((TOKEN.to_string(), n)),
        }
        self.bag_event(pi);
        n
    }

    /// Restores the saved week counter (`ilmari_tok`).
    pub(crate) fn restore_ilmari_tokens(&mut self, pid: EntityId, week: u32, earned: u32) {
        self.ilmari
            .tokens
            .insert(pid, (week, earned.min(TOKEN_WEEK_CAP)));
    }

    /// Restores the saved stamp and flag (`ilmari_at`, `ilmari_flags`). A flag without a stamp
    /// (hand-edited save) is dropped so the Opening can stamp again.
    pub(crate) fn restore_ilmari(
        &mut self,
        stamp: Option<IlmariStamp>,
        revealed: bool,
        core_full: bool,
    ) {
        self.ilmari.revealed = revealed && stamp.is_some();
        self.ilmari.core_full = core_full && stamp.is_some();
        self.ilmari.stamp = stamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_reuses_the_async_figures_full_is_authored_and_judgment_sits_on_full() {
        let raid = crate::raid::RaidDef {
            async_hp_pct: 40,
            async_damage_pct: 50,
            ..Default::default()
        };
        let jd = crate::judgment::JudgmentDef::default();
        assert_eq!(IlmariMode::Story.scale(&raid, &jd), (40, 50));
        assert_eq!(IlmariMode::Full.scale(&raid, &jd), (100, 100));
        assert_eq!(IlmariMode::Judgment.scale(&raid, &jd), (130, 125));
        let prefixes = [
            IlmariMode::Story.lock_prefix(),
            IlmariMode::Full.lock_prefix(),
            IlmariMode::Judgment.lock_prefix(),
        ];
        assert_eq!(prefixes, ["story:", "", "jd:"]);
    }

    #[test]
    fn ilmari_content_is_the_two_phases_and_the_wings() {
        for z in ["campus_occupied", "resistance/1", "ilmari1/2", "ilmari2/1"] {
            assert!(is_ilmari_zone(z), "{z}");
        }
        for z in ["campus", "parking_p3", "lab/2", "offsite/1", "tower/3"] {
            assert!(!is_ilmari_zone(z), "{z}");
        }
    }

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
