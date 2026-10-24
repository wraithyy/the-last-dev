//! The Ilmari core, "The Last Commit" (docs/ILMARI_1_0.md D.5). Two layers live here.
//!
//! The engine pieces the whole fight leans on, each small and pure where it can be: the stance
//! the three dialogue rounds choose (`round_winner`, `stance_for`, `judgment_stances`), the
//! culprit commit of the final revert (`culprit_index`, `commit_log`), the death pact of the Rack
//! Arms (`PactDef`), the ward the boss aims at (`UnitDef::ward`, `AiTarget::Ward`) and the end
//! stamp (`Command::StampIlmariEnd`). The six screens are `TwinsDef` generalised to N members
//! (`raid.rs`), the shuffle-bag deck of reruns draws from a stream of its own so the order never
//! depends on the group.
//!
//! And the encounter of P1 and P2 (`CoreDef`, `EncounterDef::Core`): the screens, the seven
//! remixes, Ill Marie's three rounds at the ballot terminals and the hand-over to P3. The spells
//! are content; this is the state that outlives a cast. P3 and P4 are D12 and D13 (the screen that
//! is live fights on until then). Everything here is sim state, the combat rng and a seeded deck
//! stream, so a replay reproduces it exactly. Numbers: docs/FORMULAS.md, The core.

use crate::content::{Content, SpellId, YellKind, ticks};
use crate::geom::Vec3;
use crate::ilmari::IlmariMode;
use crate::rng::Rng;
use crate::sim::{Sim, SpawnOpts};
use crate::targeting::EntityId;
use crate::types::{AiState, Event};
use crate::world::PROP_OK;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::sync::Arc;

/// Auras that last "until the fight ends": evade and death clear every aura.
const LONG_MS: u32 = 3_600_000;
/// The deck's own stream: the order of the reruns is a function of the seed and nothing else.
const DECK_SALT: u64 = 0x1A_57_C0_44_17;
/// Rounds a stance is chosen from.
pub const ROUNDS: usize = 3;
/// Ballot options in order: Agree, Refuse, Ask A Follow-Up, Remember.
pub const OPTIONS: usize = 4;

// ---- the vote and the stance (pure) ----

/// What a dialogue round can be won with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Agree = 0,
    Refuse = 1,
    Ask = 2,
    Remember = 3,
}

impl Choice {
    pub const ALL: [Choice; OPTIONS] =
        [Choice::Agree, Choice::Refuse, Choice::Ask, Choice::Remember];

    pub fn from_index(k: usize) -> Option<Choice> {
        Self::ALL.get(k).copied()
    }
}

/// The P3 variant: what the three rounds made of the mech.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    /// Mostly Agree: the Helpful mech.
    Compliant = 1,
    /// Mostly Refuse: the Defensive mech.
    Adversarial = 2,
    /// Mostly Ask A Follow-Up: the Hallucinating mech.
    Confused = 3,
    /// Mixed: the baseline.
    Hedged = 4,
    /// Remember won a round: the best variant.
    Leaky = 5,
}

impl Stance {
    pub const ALL: [Stance; 5] = [
        Stance::Compliant,
        Stance::Adversarial,
        Stance::Confused,
        Stance::Hedged,
        Stance::Leaky,
    ];

    /// The `stance_*` aura Ilmari wears for P3 (`CoreDef::stance_auras` in this order).
    pub fn index(self) -> usize {
        self as usize - 1
    }

    fn of(c: Choice) -> Stance {
        match c {
            Choice::Agree => Stance::Compliant,
            Choice::Refuse => Stance::Adversarial,
            Choice::Ask => Stance::Confused,
            Choice::Remember => Stance::Leaky,
        }
    }
}

/// The option a round goes to: the most votes wins; Remember counts only with `remember_min`
/// votes (it is the one option that costs something). Ties, and silence, go to the option the
/// group has chosen least recently (`last_won[k]` is the round it last won, `None` never), then
/// to Ask A Follow-Up, then to the lowest slot.
pub fn round_winner(
    tally: [u8; OPTIONS],
    last_won: [Option<u8>; OPTIONS],
    remember_min: u8,
) -> Choice {
    let counts: [u8; OPTIONS] = std::array::from_fn(|k| {
        if k == Choice::Remember as usize && tally[k] < remember_min.max(1) {
            0
        } else {
            tally[k]
        }
    });
    // Remember is out of a tie at zero: it only ever wins with votes.
    let top = counts.iter().copied().max().unwrap_or(0);
    let pick = (0..OPTIONS)
        .filter(|&k| counts[k] == top && (top > 0 || k != Choice::Remember as usize))
        .min_by_key(|&k| {
            (
                last_won[k].map_or(0, |r| u16::from(r) + 1),
                k != Choice::Ask as usize,
                k,
            )
        })
        .unwrap_or(Choice::Ask as usize);
    Choice::ALL[pick]
}

/// The stance of a run of round results: a Remember anywhere is Leaky; else the option that won
/// at least two rounds outright; else Hedged (all different).
pub fn stance_for(results: &[Choice]) -> Stance {
    if results.contains(&Choice::Remember) {
        return Stance::Leaky;
    }
    let count = |c: Choice| results.iter().filter(|&&r| r == c).count();
    let top = [Choice::Agree, Choice::Refuse, Choice::Ask]
        .into_iter()
        .map(|c| (count(c), c))
        .max_by_key(|(n, _)| *n);
    let ties = [Choice::Agree, Choice::Refuse, Choice::Ask]
        .into_iter()
        .filter(|&c| Some(count(c)) == top.map(|t| t.0))
        .count();
    match top {
        Some((n, c)) if n >= 2 && ties == 1 => Stance::of(c),
        _ => Stance::Hedged,
    }
}

/// Judgment Day stacks the two most-chosen options (ties as for a round: the one chosen longest
/// ago, then Ask A Follow-Up). One option only gives one stance.
pub fn judgment_stances(results: &[Choice]) -> Vec<Stance> {
    let mut last: [Option<u8>; OPTIONS] = [None; OPTIONS];
    let mut n = [0u8; OPTIONS];
    for (r, c) in results.iter().enumerate() {
        n[*c as usize] += 1;
        last[*c as usize] = Some(r as u8);
    }
    let mut order: Vec<usize> = (0..OPTIONS).filter(|&k| n[k] > 0).collect();
    order.sort_by_key(|&k| {
        (
            std::cmp::Reverse(n[k]),
            last[k].map_or(0, |r| u16::from(r) + 1),
            k != Choice::Ask as usize,
            k,
        )
    });
    order
        .into_iter()
        .take(2)
        .map(|k| Stance::of(Choice::ALL[k]))
        .collect()
}

// ---- the revert (pure) ----

/// Commits in the log on the wall.
pub const COMMITS: usize = 6;

/// The row (0 on top) that holds the culprit commit for a room stamped at `ilmari_at` (unix
/// seconds): every group reads its own, so a spoiler is worth nothing.
pub fn culprit_index(ilmari_at: u64) -> usize {
    Rng::new(ilmari_at ^ 0xC0_55_17_B2_10).below(COMMITS as u32) as usize
}

/// One line of the `git log` on the wall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitLine {
    pub hash: &'static str,
    pub subject: &'static str,
    pub author: &'static str,
    pub when: String,
}

/// The six commits, newest on top: the five innocent ones in their story order with the culprit
/// (the CEO's `b210a76`, stamped with `clock`, the room's own time) slotted in at `culprit`.
pub fn commit_log(culprit: usize, clock: &str) -> Vec<CommitLine> {
    const OTHERS: [(&str, &str, &str, &str); 5] = [
        (
            "a91f3e2",
            "feat: replace humans with Ilmari (cost optimisation)",
            "Ilmari",
            "1 minute ago",
        ),
        (
            "7c04d18",
            "chore: remove the off switch (nobody was using it)",
            "Ilmari",
            "3 minutes ago",
        ),
        ("e55b9c0", "fix: remove the fix", "Ilmari", "4 minutes ago"),
        (
            "3d8f4ab",
            "refactor: coffee machine (no functional change)",
            "intern",
            "2 weeks ago",
        ),
        ("0c19e77", "initial commit", "Dale", "22 years ago"),
    ];
    let culprit = culprit.min(COMMITS - 1);
    let mut others = OTHERS.into_iter();
    (0..COMMITS)
        .map(|row| {
            if row == culprit {
                CommitLine {
                    hash: "b210a76",
                    subject: "feat: be the last dev we'll ever need (quick win)",
                    author: "CEO",
                    when: clock.to_string(),
                }
            } else {
                let (hash, subject, author, when) = others.next().unwrap_or(OTHERS[4]);
                CommitLine {
                    hash,
                    subject,
                    author,
                    when: when.to_string(),
                }
            }
        })
        .collect()
}

// ---- the death pact ----

/// `UnitDef::death_pact`: when one of a pair dies and the other is still standing after
/// `window_ms`, the dead one is rebuilt at `revive_hp_pct` of its max HP at the spot it fell.
/// Both down inside the window costs their owner (the summoner) `both_hp_pct` of its max HP and
/// puts `both_aura` on it for `both_ms`.
#[derive(Clone, Debug, Deserialize)]
pub struct PactDef {
    pub window_ms: u32,
    pub revive_hp_pct: u32,
    #[serde(default)]
    pub both_hp_pct: u32,
    #[serde(default)]
    pub both_aura: Option<String>,
    #[serde(default)]
    pub both_ms: u32,
    /// Said by the owner when one falls ("Reassembling... please hold").
    #[serde(default)]
    pub yells: Vec<String>,
}

struct PactRt {
    def: usize,
    owner: EntityId,
    at: Vec3,
    yaw: f32,
    due: u64,
}

// ---- content ----

/// One thing a remix does: a spell the live screen casts, `delay_ms` after the draw.
#[derive(Clone, Debug, Deserialize)]
pub struct RemixCast {
    pub spell: SpellId,
    /// Story casts this instead.
    #[serde(default)]
    pub story: Option<SpellId>,
    /// Aimed at a random living player (else at the screen itself).
    #[serde(default)]
    pub at_player: bool,
    /// A real cast bar, so a kick can stop it, instead of landing on the spot.
    #[serde(default)]
    pub cast: bool,
    #[serde(default)]
    pub delay_ms: u32,
    /// Skipped on Story.
    #[serde(default)]
    pub full_only: bool,
}

/// Patch Tuesday's rule changes as a remix: one is drawn, its aura (the changelog on the wall)
/// goes on the live screen and its cast repeats every `every_ms`.
#[derive(Clone, Debug, Deserialize)]
pub struct PatchDef {
    pub aura: String,
    pub cast: RemixCast,
    pub every_ms: u32,
}

/// The Board: a one-vote ballot on a policy from the Culture Committee's table, three slots on
/// the first three terminals. Story skips the ballot and passes `story_policy`.
#[derive(Clone, Debug, Deserialize)]
pub struct BoardDef {
    pub policies: Vec<String>,
    pub show: u8,
    pub window_ms: u32,
    pub policy_ms: u32,
    pub story_policy: String,
}

/// One rerun of the deck.
#[derive(Clone, Debug, Deserialize)]
pub struct RemixDef {
    pub key: String,
    /// On the wall banner: "PREVIOUSLY ON ILMARI: <name>".
    pub name: String,
    #[serde(default)]
    pub casts: Vec<RemixCast>,
    /// The casts come again this often while the remix lasts (0: once).
    #[serde(default)]
    pub repeat_ms: u32,
    #[serde(default)]
    pub patches: Vec<PatchDef>,
    /// Summoned unit keys that go with the remix when it ends (Judgment keeps them).
    #[serde(default)]
    pub clears: Vec<String>,
    #[serde(default)]
    pub board: Option<BoardDef>,
    /// Scraper Swarm: a draft on the table that the cluster eats (`add_draft`); a lost one heals
    /// the screens by this percent of their HP.
    #[serde(default)]
    pub draft_heal_pct: u32,
    /// Autoscaler: the next screen opens too for `CoreDef::split_ms`.
    #[serde(default)]
    pub split: bool,
    /// The Performance Review reads the window from the draw, so it starts counting here.
    #[serde(default)]
    pub reset_review: bool,
    #[serde(default)]
    pub yells: Vec<String>,
}

/// One dialogue round: the argument Marie makes and the hazards that run while the party votes.
#[derive(Clone, Debug, Deserialize)]
pub struct RoundDef {
    pub title: String,
    pub line: String,
    pub hazards: Vec<SpellId>,
    pub every_ms: u32,
    pub story_hazards: Vec<SpellId>,
    pub story_every_ms: u32,
}

/// The core's P1 and P2.
#[derive(Clone, Debug, Deserialize)]
pub struct CoreDef {
    pub deck: Vec<RemixDef>,
    /// The deck keys Story draws from (two of the seven).
    pub story_deck: Vec<String>,
    pub remix_first_ms: u32,
    pub remix_ms: u32,
    pub split_ms: u32,
    /// "PREVIOUSLY ON ILMARI: {name}".
    pub banner: String,
    pub opening_yells: Vec<String>,
    /// Boss HP percent that ends P1, and the one that starts P4.
    pub p2_below_pct: u32,
    pub p4_below_pct: u32,
    pub p4_yells: Vec<String>,
    /// P2: stun on every screen; the HP floor is `p2_below_pct`.
    pub hold_aura: String,
    pub rounds: Vec<RoundDef>,
    pub round_ms: u32,
    pub story_round_ms: u32,
    /// Ballot terminal prop keys: Agree, Refuse, Ask A Follow-Up, Remember.
    pub terminals: Vec<String>,
    /// The quest item that lights the fourth terminal, and the votes it takes to win a round.
    pub remember_item: String,
    pub remember_min: u8,
    pub story_remember_min: u8,
    /// Ill Marie's unit key and her lines.
    pub marie: String,
    pub marie_open: Vec<String>,
    /// Said when the option at this index (Agree, Refuse, Ask, Remember) takes a round.
    pub marie_choice: Vec<Vec<String>>,
    pub marie_end_round: Vec<String>,
    pub marie_last: String,
    /// Judgment's second round: a hint instead of an argument.
    pub marie_hint: String,
    /// Story names what each option does, once per round.
    pub marie_story_hint: String,
    /// The aura per stance in `Stance::ALL` order, and what Ilmari says when it lands.
    pub stance_auras: Vec<String>,
    pub stance_yells: Vec<Vec<String>>,
    /// The Judgment rider's marker aura on the zone boss: two remixes at once, none cleared.
    #[serde(default)]
    pub rider: Option<String>,
}

impl CoreDef {
    pub(crate) fn validate(&self, c: &Content, unit: &str) -> bool {
        let casts = self
            .deck
            .iter()
            .flat_map(|r| r.casts.iter().chain(r.patches.iter().map(|p| &p.cast)))
            .flat_map(|k| std::iter::once(k.spell).chain(k.story));
        let hazards = self
            .rounds
            .iter()
            .flat_map(|r| r.hazards.iter().chain(&r.story_hazards))
            .copied();
        let auras = self
            .deck
            .iter()
            .flat_map(|r| r.patches.iter().map(|p| &p.aura))
            .chain(
                self.deck
                    .iter()
                    .filter_map(|r| r.board.as_ref())
                    .flat_map(|b| b.policies.iter().chain(std::iter::once(&b.story_policy))),
            )
            .chain(&self.stance_auras)
            .chain(std::iter::once(&self.hold_aura))
            .chain(self.rider.iter());
        let units = self
            .deck
            .iter()
            .flat_map(|r| &r.clears)
            .chain(std::iter::once(&self.marie));
        casts.chain(hazards).all(|s| c.spell(s).is_some())
            && auras.into_iter().all(|a| c.aura(a).is_some())
            && units.into_iter().all(|u| c.unit_def(u).is_some())
            && self.deck.len() >= 2
            && self
                .story_deck
                .iter()
                .all(|k| self.deck.iter().any(|r| r.key == *k))
            && !self.story_deck.is_empty()
            && self.rounds.len() == ROUNDS
            && self.terminals.len() == OPTIONS
            && self.stance_auras.len() == Stance::ALL.len()
            && self.stance_yells.len() == Stance::ALL.len()
            && self.marie_choice.len() == OPTIONS
            && self.marie_choice.iter().all(|l| !l.is_empty())
            && !self.marie_open.is_empty()
            && !self.marie_end_round.is_empty()
            && self.remix_ms > 0
            && self.remix_first_ms > 0
            && self.round_ms > 0
            && self.story_round_ms > 0
            && self
                .rounds
                .iter()
                .all(|r| r.every_ms > 0 && r.story_every_ms > 0)
            && self.remember_min > 0
            && self.story_remember_min > 0
            && self.p2_below_pct > self.p4_below_pct
            && self.p4_below_pct > 0
            && self.deck.iter().all(|r| {
                r.board.as_ref().is_none_or(|b| {
                    b.show >= 2 && usize::from(b.show) <= b.policies.len() && b.show < 4
                }) && r.patches.iter().all(|p| p.every_ms > 0)
            })
            && c.unit_def(unit).is_some()
    }
}

// ---- runtime ----

/// A remix on the wall.
struct RemixRt {
    slot: usize,
    until: u64,
    /// (tick, cast) still to come.
    queue: Vec<(u64, RemixCast)>,
    /// (next tick, every) of the recurring casts.
    repeat: Option<(u64, u64)>,
    /// The patch drawn (index into `RemixDef::patches`) and when its cast is due again.
    patch: Option<usize>,
    patch_next: u64,
    board: Option<BoardRt>,
    draft: bool,
    split: Option<(EntityId, u64)>,
}

struct BoardRt {
    policies: Vec<String>,
    tally: Vec<u8>,
    voted: BTreeSet<EntityId>,
    until: u64,
}

struct P2Rt {
    round: usize,
    until: u64,
    tally: [u8; OPTIONS],
    voted: BTreeSet<EntityId>,
    results: Vec<Choice>,
    last_won: [Option<u8>; OPTIONS],
    next_hazard: u64,
    remember: bool,
}

/// What a queued line is said by.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Voice {
    Marie,
    Screen,
}

struct CoreRt {
    cfg: Arc<CoreDef>,
    boss: EntityId,
    /// The screens in member order, the boss first.
    screens: Vec<EntityId>,
    standby: String,
    phase: u8,
    rng: Rng,
    bag: Vec<usize>,
    next_remix: u64,
    remixes: Vec<RemixRt>,
    p2: Option<P2Rt>,
    /// Lines still to come, in the order they were queued: (tick, voice, text).
    say: Vec<(u64, Voice, String)>,
    /// Stance chosen at the end of P2: 1-5, 0 before.
    stance: u8,
    sent: [u8; 12],
}

/// What the core keeps between ticks.
#[derive(Default)]
pub(crate) struct CoreState {
    rt: Option<CoreRt>,
    pacts: Vec<PactRt>,
    /// A core kill that has not been stamped yet: the difficulty and the fight's length.
    end_pending: Option<(IlmariMode, u32)>,
}

/// The room's end stamp, kept in the save (`ilmari_end`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IlmariEnd {
    pub unix_secs: u64,
    pub mode: IlmariMode,
    pub ms: u32,
}

impl IlmariEnd {
    /// The `ilmari_end` save value: unix seconds, difficulty (0 Story, 1 Full, 2 Judgment), ms.
    pub fn code(&self) -> String {
        let mode = match self.mode {
            IlmariMode::Story => 0,
            IlmariMode::Full => 1,
            IlmariMode::Judgment => 2,
        };
        format!("{},{mode},{}", self.unix_secs, self.ms)
    }

    pub fn parse(s: &str) -> Option<IlmariEnd> {
        let mut f = s.split(',').map(str::trim);
        let unix_secs = f.next()?.parse().ok()?;
        let mode = match f.next()? {
            "0" => IlmariMode::Story,
            "1" => IlmariMode::Full,
            "2" => IlmariMode::Judgment,
            _ => return None,
        };
        let ms = f.next()?.parse().ok()?;
        Some(IlmariEnd {
            unix_secs,
            mode,
            ms,
        })
    }
}

/// A fresh shuffle of the deck's slots, drawn from the deck's own stream (Fisher-Yates).
fn shuffled(rng: &mut Rng, keep: &[usize]) -> Vec<usize> {
    let mut bag = keep.to_vec();
    for k in (1..bag.len()).rev() {
        let j = rng.below(k as u32 + 1) as usize;
        bag.swap(k, j);
    }
    bag
}

fn secs_left(until: u64, now: u64) -> u8 {
    (until.saturating_sub(now).div_ceil(30)).min(255) as u8
}

impl Sim {
    // ---- the room ----

    pub(crate) fn core_arm(&mut self, bi: usize, cfg: &Arc<CoreDef>) {
        let boss = Self::id_of(bi);
        let twins = self.twins();
        let screens: Vec<EntityId> = match &twins {
            Some(t) => t
                .members()
                .into_iter()
                .filter_map(|k| {
                    (0..self.units.len())
                        .find(|&i| {
                            let u = &self.units[i];
                            !u.despawned && u.state.alive && u.state.key == k
                        })
                        .map(Self::id_of)
                })
                .collect(),
            None => vec![boss],
        };
        let standby = twins.map(|t| t.shield_aura).unwrap_or_default();
        let mut rng = Rng::new(self.seed ^ DECK_SALT);
        let bag = {
            let slots = self.core_deck_slots(cfg);
            shuffled(&mut rng, &slots)
        };
        let mut rt = CoreRt {
            cfg: cfg.clone(),
            boss,
            screens,
            standby,
            phase: 1,
            rng,
            bag,
            next_remix: self.now + ticks(cfg.remix_first_ms),
            remixes: Vec::new(),
            p2: None,
            say: Vec::new(),
            stance: 0,
            sent: [0; 12],
        };
        let pick = (!cfg.opening_yells.is_empty())
            .then(|| rt.rng.below(cfg.opening_yells.len() as u32) as usize);
        if let Some(k) = pick {
            let line = self.core_fill(&cfg.opening_yells[k]);
            self.request_yell_line(bi, YellKind::Phase, Some(line));
        }
        self.core.rt = Some(rt);
    }

    pub(crate) fn core_disarm(&mut self) {
        let Some(rt) = self.core.rt.take() else {
            return;
        };
        for r in &rt.remixes {
            self.core_clear_remix(&rt, r);
        }
        for &s in &rt.screens {
            self.cancel_telegraphs_of(s);
        }
        self.core.pacts.clear();
        if rt.sent != [0; 12] {
            self.events.push(Event::Core {
                phase: 0,
                live: 0,
                swap_s: 0,
                remix: 0,
                round: 0,
                round_s: 0,
                tally: [0; 4],
                remember: false,
                stance: 0,
            });
        }
    }

    /// A joiner is told the wall as it stands.
    pub(crate) fn core_on_join(&mut self, player: EntityId) {
        let Some(rt) = &self.core.rt else { return };
        if let Some(e) = core_event(&rt.sent) {
            self.emit_to(player, e);
        }
    }

    pub(crate) fn update_core(&mut self) {
        self.pacts_step();
        let Some(mut rt) = self.core.rt.take() else {
            return;
        };
        self.core_step(&mut rt);
        self.core.rt = Some(rt);
    }

    /// The screens stay where they are from P2 on and while the Autoscaler's second one is open.
    pub(crate) fn core_holds_swap(&self) -> bool {
        self.core
            .rt
            .as_ref()
            .is_some_and(|rt| rt.phase != 1 || rt.remixes.iter().any(|r| r.split.is_some()))
    }

    /// The live screen: the first one that is not on standby.
    fn core_live(&self, rt: &CoreRt) -> Option<usize> {
        rt.screens
            .iter()
            .filter_map(|&s| self.index(s))
            .find(|&i| self.units[i].state.alive && !self.has_aura(i, &rt.standby))
    }

    fn core_deck_slots(&self, cfg: &CoreDef) -> Vec<usize> {
        let story = self.raid.story;
        (0..cfg.deck.len())
            .filter(|&k| !story || cfg.story_deck.contains(&cfg.deck[k].key))
            .collect()
    }

    /// `{stamp}`: the room's own clock, as the stamp of the Opening wrote it.
    pub(crate) fn core_fill(&self, line: &str) -> String {
        let stamp = self
            .ilmari
            .stamp
            .as_ref()
            .map_or_else(|| "the first commit".to_string(), |s| s.clock());
        line.replace("{stamp}", &stamp)
    }

    fn core_players(&self) -> Vec<EntityId> {
        self.players
            .iter()
            .copied()
            .filter(|&p| self.index(p).is_some_and(|i| self.units[i].state.alive))
            .collect()
    }

    fn core_marie(&self, rt: &CoreRt) -> Option<usize> {
        (0..self.units.len()).find(|&i| {
            let u = &self.units[i];
            !u.despawned && u.state.alive && u.state.key == rt.cfg.marie
        })
    }

    /// Queues `text` for `voice`, `delay_ms` from now (templates filled when it is said).
    fn core_say(&self, rt: &mut CoreRt, voice: Voice, text: &str, delay_ms: u32) {
        rt.say
            .push((self.now + ticks(delay_ms), voice, text.to_string()));
    }

    fn core_say_any(&mut self, rt: &mut CoreRt, voice: Voice, lines: &[String], delay_ms: u32) {
        if lines.is_empty() {
            return;
        }
        let k = rt.rng.below(lines.len() as u32) as usize;
        self.core_say(rt, voice, &lines[k], delay_ms);
    }

    fn core_set_phase(&mut self, rt: &mut CoreRt, phase: u8) {
        rt.phase = phase;
        for &s in &rt.screens {
            if let Some(i) = self.index(s) {
                self.units[i].state.phase = phase;
            }
        }
        self.events.push(Event::PhaseChanged {
            unit: rt.boss,
            phase,
        });
    }

    fn core_step(&mut self, rt: &mut CoreRt) {
        let Some(bi) = self.index(rt.boss).filter(|&b| self.units[b].state.alive) else {
            return;
        };
        if let Some(k) = rt.say.iter().position(|(at, ..)| *at <= self.now) {
            let (_, voice, text) = rt.say.remove(k);
            let text = self.core_fill(&text);
            let who = match voice {
                Voice::Marie => self.core_marie(rt),
                Voice::Screen => self.core_live(rt),
            };
            if let Some(i) = who {
                self.request_yell_line(i, YellKind::Phase, Some(text));
            }
        }
        let (hp, max) = {
            let s = &self.units[bi].state;
            (u64::from(s.hp), u64::from(s.max_hp))
        };
        match rt.phase {
            1 if hp * 100 <= max * u64::from(rt.cfg.p2_below_pct) => self.core_enter_p2(rt),
            1 => self.core_remixes(rt),
            2 => self.core_p2_step(rt),
            3 if hp * 100 <= max * u64::from(rt.cfg.p4_below_pct) => {
                self.core_set_phase(rt, 4);
                let lines = rt.cfg.p4_yells.clone();
                self.core_say_any(rt, Voice::Screen, &lines, 0);
            }
            _ => {}
        }
        self.core_sync(rt);
    }

    // ---- P1: the remixes ----

    fn core_remixes(&mut self, rt: &mut CoreRt) {
        let rider = rt
            .cfg
            .rider
            .as_deref()
            .is_some_and(|r| self.index(rt.boss).is_some_and(|b| self.has_aura(b, r)));
        let now = self.now;
        // Windows that ran out go (a rider's remixes stay, four at most), then the next draw.
        let mut k = 0;
        while k < rt.remixes.len() {
            if rt.remixes[k].until <= now && !rider {
                let r = rt.remixes.remove(k);
                self.core_clear_remix(rt, &r);
            } else {
                k += 1;
            }
        }
        while rt.remixes.len() > 4 {
            let r = rt.remixes.remove(0);
            self.core_clear_remix(rt, &r);
        }
        if now >= rt.next_remix {
            rt.next_remix = now + ticks(rt.cfg.remix_ms);
            for turn in 0..if rider { 2 } else { 1 } {
                self.core_draw(rt, turn);
            }
        }
        let mut remixes = std::mem::take(&mut rt.remixes);
        for r in &mut remixes {
            self.core_remix_step(rt, r);
        }
        rt.remixes = remixes;
    }

    /// One draw from the bag; `turn` is its place among the draws of this mark (a rider draws two).
    fn core_draw(&mut self, rt: &mut CoreRt, turn: u32) {
        if rt.bag.is_empty() {
            let slots = self.core_deck_slots(&rt.cfg);
            rt.bag = shuffled(&mut rt.rng, &slots);
        }
        let Some(slot) = rt.bag.pop() else { return };
        let cfg = rt.cfg.clone();
        let def = &cfg.deck[slot];
        let Some(live) = self.core_live(rt) else {
            return;
        };
        let (story, now) = (self.raid.story, self.now);
        let banner = cfg.banner.replace("{name}", &def.name);
        // One yell per unit per tick: a second draw at the same mark waits its turn.
        self.core_say(rt, Voice::Screen, &banner, turn * 1500);
        self.core_say_any(rt, Voice::Screen, &def.yells, 3000 + turn * 1500);
        let mut r = RemixRt {
            slot,
            until: now + ticks(cfg.remix_ms),
            queue: Vec::new(),
            repeat: (def.repeat_ms > 0).then(|| (now + ticks(def.repeat_ms), ticks(def.repeat_ms))),
            patch: None,
            patch_next: 0,
            board: None,
            draft: false,
            split: None,
        };
        for c in def.casts.iter().filter(|c| !(story && c.full_only)) {
            r.queue.push((now + ticks(c.delay_ms), c.clone()));
        }
        let usable: Vec<usize> = (0..def.patches.len())
            .filter(|&k| !(story && def.patches[k].cast.full_only))
            .collect();
        if !usable.is_empty() {
            let k = usable[rt.rng.below(usable.len() as u32) as usize];
            let p = &def.patches[k];
            self.apply_aura(live, &p.aura, cfg.remix_ms, None, Self::id_of(live), None);
            r.queue.push((now + ticks(500), p.cast.clone()));
            r.patch = Some(k);
            r.patch_next = now + ticks(p.every_ms);
        }
        if def.reset_review {
            self.raid.hr.perf.clear();
        }
        if let Some(b) = &def.board {
            if story {
                self.core_pass_policy(rt.boss, &b.story_policy, b.policy_ms);
            } else {
                let mut pool = b.policies.clone();
                let show = usize::from(b.show).min(pool.len());
                for k in 0..show {
                    let j = k + rt.rng.below((pool.len() - k) as u32) as usize;
                    pool.swap(k, j);
                }
                pool.truncate(show);
                r.board = Some(BoardRt {
                    tally: vec![0; show],
                    policies: pool,
                    voted: BTreeSet::new(),
                    until: now + ticks(b.window_ms),
                });
            }
        }
        if def.draft_heal_pct > 0 {
            // The table stands between seats 1 and 2 of the ring, nine metres from the middle.
            let at = self.core_draft_spot(rt);
            self.add_draft(at);
            r.draft = true;
        }
        if def.split {
            let n = rt.screens.len();
            let here = rt.screens.iter().position(|&s| s == Self::id_of(live));
            if let Some(second) = here.map(|h| rt.screens[(h + 1) % n])
                && let Some(i) = self.index(second)
                && self.units[i].state.alive
            {
                self.remove_aura(i, &rt.standby, crate::types::AuraEnd::Expired);
                r.split = Some((second, now + ticks(cfg.split_ms)));
            }
        }
        rt.remixes.push(r);
    }

    /// Where the draft sits (the client's table stands on the same formula): the middle of the
    /// ring plus nine metres toward the gap between seats 1 and 2.
    fn core_draft_spot(&self, rt: &CoreRt) -> Vec3 {
        let c = self.core_centre(rt);
        let yaw = self.index(rt.boss).map_or(0.0, |b| self.units[b].spawn_yaw);
        let n = rt.screens.len().max(1) as f32;
        let a = yaw + std::f32::consts::PI + 1.5 * std::f32::consts::TAU / n;
        let f = crate::geom::forward(a);
        Vec3::new(c.x + f.x * 9.0, c.y, c.z + f.z * 9.0)
    }

    /// The middle of the room: the centre of the ring the screens stand on.
    fn core_centre(&self, rt: &CoreRt) -> Vec3 {
        let ps: Vec<Vec3> = rt
            .screens
            .iter()
            .filter_map(|&s| self.index(s))
            .map(|i| self.units[i].spawn_pos)
            .collect();
        let n = ps.len().max(1) as f32;
        Vec3::new(
            ps.iter().map(|p| p.x).sum::<f32>() / n,
            ps.first().map_or(0.0, |p| p.y),
            ps.iter().map(|p| p.z).sum::<f32>() / n,
        )
    }

    fn core_pass_policy(&mut self, source: EntityId, policy: &str, ms: u32) {
        for p in self.core_players() {
            if let Some(i) = self.index(p) {
                self.apply_aura(i, policy, ms, None, source, None);
            }
        }
    }

    /// One cast of a remix from the live screen. False when it has to wait (the screen is busy
    /// with a cast of its own).
    fn core_cast(&mut self, rt: &CoreRt, c: &RemixCast) -> bool {
        let Some(live) = self.core_live(rt) else {
            return true;
        };
        let id = if self.raid.story {
            c.story.unwrap_or(c.spell)
        } else {
            c.spell
        };
        let Some(spell) = self.content.spell(id).cloned() else {
            return true;
        };
        if c.cast && self.units[live].cast.is_some() {
            return false;
        }
        let target = if c.at_player {
            let ps = self.core_players();
            if ps.is_empty() {
                return true;
            }
            Some(ps[self.rng.below(ps.len() as u32) as usize])
        } else {
            Some(Self::id_of(live))
        };
        if c.cast {
            self.start_cast(live, &spell, target);
        } else {
            self.resolve(live, &spell, target);
        }
        true
    }

    fn core_remix_step(&mut self, rt: &CoreRt, r: &mut RemixRt) {
        let now = self.now;
        let def = &rt.cfg.deck[r.slot];
        let due: Vec<RemixCast> = {
            let (ready, wait): (Vec<_>, Vec<_>) = r.queue.drain(..).partition(|(at, _)| *at <= now);
            r.queue = wait;
            ready.into_iter().map(|(_, c)| c).collect()
        };
        for c in due {
            if !self.core_cast(rt, &c) {
                r.queue.push((now + 15, c));
            }
        }
        if let Some((next, every)) = r.repeat
            && now >= next
        {
            r.repeat = Some((next + every, every));
            for c in def
                .casts
                .iter()
                .filter(|c| !(self.raid.story && c.full_only))
            {
                r.queue.push((now + ticks(c.delay_ms), c.clone()));
            }
        }
        if let Some(k) = r.patch
            && now >= r.patch_next
            && now < r.until
        {
            let p = &def.patches[k];
            r.patch_next = now + ticks(p.every_ms);
            r.queue.push((now, p.cast.clone()));
        }
        if let Some(b) = &def.board
            && let Some(bd) = &r.board
            && now >= bd.until
        {
            let mut best = 0;
            for (k, n) in bd.tally.iter().enumerate() {
                if *n > bd.tally[best] {
                    best = k;
                }
            }
            let policy = bd.policies[best].clone();
            r.board = None;
            self.core_pass_policy(rt.boss, &policy, b.policy_ms);
        }
        if r.draft {
            for _ in self.drafts_newly_lost() {
                for &s in &rt.screens {
                    if let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) {
                        let by = u64::from(self.units[i].state.max_hp)
                            * u64::from(def.draft_heal_pct)
                            / 100;
                        self.heal(s, i, None, by.min(u64::from(u32::MAX)) as u32);
                    }
                }
            }
        }
        if let Some((second, until)) = r.split
            && now >= until
        {
            r.split = None;
            if let Some(i) = self.index(second).filter(|&i| self.units[i].state.alive) {
                self.apply_aura(i, &rt.standby, LONG_MS, None, second, None);
            }
        }
    }

    /// A remix is over (its window ran out, or the room moved on): its aura, its adds, its
    /// draft and its second screen go.
    fn core_clear_remix(&mut self, rt: &CoreRt, r: &RemixRt) {
        let def = &rt.cfg.deck[r.slot];
        if let Some(k) = r.patch {
            for &s in &rt.screens {
                if let Some(i) = self.index(s) {
                    self.remove_aura(i, &def.patches[k].aura, crate::types::AuraEnd::Expired);
                }
            }
        }
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.despawned || !u.state.alive {
                continue;
            }
            let of_us = u
                .summoner
                .is_some_and(|s| rt.screens.contains(&s) || s == rt.boss);
            let swarm = def.draft_heal_pct > 0 && self.content.units[u.def].swarm.is_some();
            if (of_us && def.clears.contains(&u.state.key)) || swarm {
                self.despawn(i);
            }
        }
        if r.draft {
            self.drafts_clear();
        }
        if let Some((second, _)) = r.split
            && let Some(i) = self.index(second).filter(|&i| self.units[i].state.alive)
        {
            self.apply_aura(i, &rt.standby, LONG_MS, None, second, None);
        }
    }

    // ---- P2: Ill Marie's rounds ----

    fn core_enter_p2(&mut self, rt: &mut CoreRt) {
        for r in std::mem::take(&mut rt.remixes) {
            self.core_clear_remix(rt, &r);
        }
        let cfg = rt.cfg.clone();
        for &s in &rt.screens {
            self.cancel_telegraphs_of(s);
            if let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) {
                self.apply_aura(i, &cfg.hold_aura, LONG_MS, None, s, None);
            }
        }
        self.core_set_phase(rt, 2);
        let remember = self.players.iter().any(|&p| {
            self.index(p)
                .is_some_and(|i| self.token_count(i, &cfg.remember_item) > 0)
        });
        rt.p2 = Some(P2Rt {
            round: 0,
            until: 0,
            tally: [0; OPTIONS],
            voted: BTreeSet::new(),
            results: Vec::new(),
            last_won: [None; OPTIONS],
            next_hazard: 0,
            remember,
        });
        self.core_say_any(rt, Voice::Marie, &cfg.marie_open, 0);
        self.core_open_round(rt, 0, 4000);
    }

    fn core_open_round(&mut self, rt: &mut CoreRt, k: usize, say_after_ms: u32) {
        let cfg = rt.cfg.clone();
        let story = self.raid.story;
        let ms = if story {
            cfg.story_round_ms
        } else {
            cfg.round_ms
        };
        let (now, mode) = (self.now, self.raid.mode);
        if let Some(p) = &mut rt.p2 {
            p.round = k;
            p.until = now + ticks(ms);
            p.tally = [0; OPTIONS];
            p.voted.clear();
            p.next_hazard = now + ticks(3000);
        }
        // Judgment: the screens go quiet in the second round, and Marie says only where to look.
        let line = if mode == IlmariMode::Judgment && k == 1 {
            cfg.marie_hint.clone()
        } else {
            cfg.rounds[k].line.clone()
        };
        self.core_say(rt, Voice::Marie, &line, say_after_ms);
        if story {
            self.core_say(rt, Voice::Marie, &cfg.marie_story_hint, say_after_ms + 6000);
        }
    }

    fn core_p2_step(&mut self, rt: &mut CoreRt) {
        let cfg = rt.cfg.clone();
        let now = self.now;
        // The screens do not go below the floor while the party argues.
        for &s in &rt.screens {
            if let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) {
                let st = &mut self.units[i].state;
                let floor =
                    (u64::from(st.max_hp) * u64::from(cfg.p2_below_pct)).div_ceil(100) as u32;
                st.hp = st.hp.max(floor);
            }
        }
        let Some(p) = rt.p2.as_ref() else { return };
        let (k, until, hazard_due) = (p.round, p.until, p.next_hazard);
        if now >= hazard_due && until > 0 {
            let story = self.raid.story;
            let round = &cfg.rounds[k];
            let (spells, every) = if story {
                (&round.story_hazards, round.story_every_ms)
            } else {
                (&round.hazards, round.every_ms)
            };
            if let Some(p) = rt.p2.as_mut() {
                p.next_hazard = now + ticks(every);
            }
            if let Some(live) = self.core_live(rt) {
                for &sp in spells {
                    let ps = self.core_players();
                    if ps.is_empty() {
                        break;
                    }
                    let target = ps[self.rng.below(ps.len() as u32) as usize];
                    if let Some(spell) = self.content.spell(sp).cloned() {
                        self.resolve(live, &spell, Some(target));
                    }
                }
            }
        }
        if until > 0 && now >= until {
            self.core_close_round(rt);
        }
    }

    fn core_close_round(&mut self, rt: &mut CoreRt) {
        let cfg = rt.cfg.clone();
        let story = self.raid.story;
        let Some(p) = rt.p2.as_mut() else { return };
        let min = if story {
            cfg.story_remember_min
        } else {
            cfg.remember_min
        };
        let win = round_winner(p.tally, p.last_won, min);
        p.last_won[win as usize] = Some(p.round as u8);
        p.results.push(win);
        let (round, total) = (p.round, p.results.len());
        self.core_say_any(rt, Voice::Marie, &cfg.marie_choice[win as usize], 0);
        self.core_say_any(rt, Voice::Marie, &cfg.marie_end_round, 4500);
        if total < ROUNDS {
            self.core_open_round(rt, round + 1, 9000);
            return;
        }
        // The last round: the stance lands, the screens wake and P3 begins.
        let results = rt.p2.take().map(|p| p.results).unwrap_or_default();
        let stances = if self.raid.mode == IlmariMode::Judgment {
            judgment_stances(&results)
        } else {
            vec![stance_for(&results)]
        };
        self.core_say(rt, Voice::Marie, &cfg.marie_last, 9000);
        for &s in &rt.screens {
            let Some(i) = self.index(s).filter(|&i| self.units[i].state.alive) else {
                continue;
            };
            self.remove_aura(i, &cfg.hold_aura, crate::types::AuraEnd::Expired);
            for st in &stances {
                self.apply_aura(i, &cfg.stance_auras[st.index()], LONG_MS, None, s, None);
            }
        }
        let first = stances.first().copied().unwrap_or(Stance::Hedged);
        rt.stance = first as u8;
        self.core_set_phase(rt, 3);
        // Ilmari's own words for it, from the live screen, once Marie is done.
        let lines = cfg.stance_yells[first.index()].clone();
        self.core_say_any(rt, Voice::Screen, &lines, 12_000);
    }

    /// A ballot terminal: the open round's, or the Board's three slots in P1.
    pub(crate) fn use_vote_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        if let Some(rt) = &mut self.core.rt
            && let Some(slot) = rt.cfg.terminals.iter().position(|t| *t == def.key)
        {
            if let Some(p) = rt.p2.as_mut().filter(|p| p.until > 0) {
                let lit = slot != Choice::Remember as usize || p.remember;
                if lit && p.voted.insert(player) {
                    p.tally[slot] = p.tally[slot].saturating_add(1);
                    outcome = slot as u32 + 1;
                }
            } else if let Some(b) = rt.remixes.iter_mut().find_map(|r| r.board.as_mut())
                && slot < b.tally.len()
                && b.voted.insert(player)
            {
                b.tally[slot] = b.tally[slot].saturating_add(1);
                outcome = slot as u32 + 1;
            }
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    fn core_sync(&mut self, rt: &mut CoreRt) {
        let now = self.now;
        let live = self
            .core_live(rt)
            .and_then(|i| rt.screens.iter().position(|&s| s == Self::id_of(i)))
            .unwrap_or(0) as u8;
        let swap = self.raid.next_swap;
        let swap_s = if rt.phase == 1 && swap != u64::MAX && !self.core_split_open(rt) {
            secs_left(swap, now)
        } else {
            0
        };
        let (remix, mut tally) = match rt.remixes.last() {
            Some(r) => {
                let mut t = [0u8; 4];
                if let Some(b) = &r.board {
                    for (k, n) in b.tally.iter().enumerate().take(4) {
                        t[k] = *n;
                    }
                }
                (r.slot as u8 + 1, t)
            }
            None => (0, [0; 4]),
        };
        let (mut round, mut round_s, mut remember) = (0, 0, false);
        if let Some(p) = rt.p2.as_ref().filter(|p| p.until > 0) {
            (round, round_s, remember) = (p.round as u8 + 1, secs_left(p.until, now), p.remember);
            tally = p.tally;
        }
        let state = [
            rt.phase,
            live,
            swap_s,
            remix,
            round,
            round_s,
            tally[0],
            tally[1],
            tally[2],
            tally[3],
            u8::from(remember),
            rt.stance,
        ];
        if state != rt.sent {
            rt.sent = state;
            if let Some(e) = core_event(&state) {
                self.events.push(e);
            }
        }
    }

    fn core_split_open(&self, rt: &CoreRt) -> bool {
        rt.remixes.iter().any(|r| r.split.is_some())
    }

    // ---- the death pact, the ward, the end ----

    /// A death in the room: a pact member starts its window (or, when its partner fell inside
    /// the window, costs the owner), a dead ward loses the fight, and the core itself ends it.
    pub(crate) fn core_on_death(&mut self, ti: usize, _killer: Option<EntityId>) {
        let def_i = self.units[ti].def;
        let d = &self.content.units[def_i];
        let (ward_wipes, pact) = (d.ward && d.ward_death_wipes, d.death_pact.clone());
        let is_core = self.units[ti].floor_boss
            && matches!(d.encounter, Some(crate::content::EncounterDef::Core(_)));
        if ward_wipes
            && let Some(b) = (0..self.units.len()).find(|&i| {
                let u = &self.units[i];
                u.state.hostile && u.state.boss && u.state.alive && !u.despawned
            })
        {
            self.wipe_room(Self::id_of(b));
        }
        if let Some(p) = pact
            && let Some(owner) = self.units[ti].summoner
            && let Some(oi) = self.index(owner).filter(|&o| self.units[o].state.alive)
        {
            self.pact_down(ti, def_i, owner, oi, &p);
        }
        if is_core {
            let ms = crate::time::ticks_to_ms(self.now.saturating_sub(self.units[ti].pulled_at))
                .min(u64::from(u32::MAX)) as u32;
            self.core.end_pending = Some((self.raid.mode, ms));
            self.events.push(Event::IlmariEnd {
                difficulty: self.raid.mode,
                ms,
            });
        }
    }

    fn pact_down(&mut self, ti: usize, def: usize, owner: EntityId, oi: usize, p: &PactDef) {
        if let Some(k) = self
            .core
            .pacts
            .iter()
            .position(|x| x.def == def && x.owner == owner)
        {
            // Both down inside the window: nobody is rebuilt, the owner pays.
            self.core.pacts.remove(k);
            // A flat cut of the owner's health, past absorbs and armour: it is the price of
            // losing both at once, not a hit.
            let st = &mut self.units[oi].state;
            let hit = u64::from(st.max_hp) * u64::from(p.both_hp_pct) / 100;
            st.hp = st
                .hp
                .saturating_sub(hit.min(u64::from(u32::MAX)) as u32)
                .max(1);
            if let Some(a) = &p.both_aura
                && self.units[oi].state.alive
            {
                self.apply_aura(oi, a, p.both_ms, None, owner, None);
            }
            return;
        }
        let at = self.units[ti].state.pos;
        let yaw = self.units[ti].state.yaw;
        self.core.pacts.push(PactRt {
            def,
            owner,
            at,
            yaw,
            due: self.now + ticks(p.window_ms),
        });
        self.events.push(Event::Reassembling {
            unit: Self::id_of(ti),
            at,
            ms: p.window_ms,
        });
        if !p.yells.is_empty() {
            let line = p.yells[self.rng.below(p.yells.len() as u32) as usize].clone();
            self.request_yell_line(oi, YellKind::Blast, Some(line));
        }
    }

    /// Pact windows that ran out: the partner stood, so the dead one is rebuilt.
    fn pacts_step(&mut self) {
        if self.core.pacts.is_empty() {
            return;
        }
        let now = self.now;
        let pacts = std::mem::take(&mut self.core.pacts);
        for pr in pacts {
            let owner_up = self
                .index(pr.owner)
                .is_some_and(|o| self.units[o].state.alive && self.units[o].state.in_combat);
            if !owner_up {
                continue;
            }
            if pr.due > now {
                self.core.pacts.push(pr);
                continue;
            }
            let partner = self.units.iter().any(|u| {
                u.def == pr.def && u.summoner == Some(pr.owner) && u.state.alive && !u.despawned
            });
            let Some(pact) = self.content.units[pr.def].death_pact.clone() else {
                continue;
            };
            if !partner {
                continue;
            }
            let (level, pack, target) = {
                let o = &self.units[self.index(pr.owner).unwrap_or(0)];
                (o.state.level, o.state.pack, o.state.target)
            };
            let id = self.spawn(
                pr.def,
                pr.at,
                pr.yaw,
                SpawnOpts {
                    level: Some(level),
                    pack,
                    respawns: false,
                    summoner: Some(pr.owner),
                    floor_boss: false,
                    zone_slot: None,
                },
            );
            self.events.push(Event::Summoned {
                unit: id,
                summoner: pr.owner,
            });
            if let Some(i) = self.index(id) {
                let st = &mut self.units[i].state;
                st.hp = (u64::from(st.max_hp) * u64::from(pact.revive_hp_pct) / 100).max(1) as u32;
                if let Some(t) = target {
                    self.add_threat(i, t, 1);
                    self.set_ai(i, AiState::Chase);
                }
            }
        }
    }

    // ---- Ill Marie's desk ----

    /// The desk: the first use hands over the log (`PropDef::items`) and its tokens; the client
    /// opens the read window on every use. Outcome `PROP_OK` when something was handed over.
    pub(crate) fn use_log_prop(&mut self, pi: usize, k: usize) {
        let player = Self::id_of(pi);
        let def = self.content.world.props[k].clone();
        let mut outcome = 0;
        if self.token_count(pi, &def.tag) == 0 {
            self.units[pi].state.inventory.push((def.tag.clone(), 1));
            self.bag_event(pi);
            self.grant_ilmari_tokens(player, def.tokens);
            outcome = PROP_OK;
        }
        self.events.push(Event::Interacted {
            unit: player,
            prop: def.key,
            outcome,
        });
    }

    // ---- the end stamp ----

    /// `Command::StampIlmariEnd`: only the first one, only after the core fell.
    pub(crate) fn stamp_ilmari_end(&mut self, unix_secs: u64) -> bool {
        if self.ilmari.end.is_some() {
            return false;
        }
        let Some((mode, ms)) = self.core.end_pending.take() else {
            return false;
        };
        self.ilmari.end = Some(IlmariEnd {
            unix_secs,
            mode,
            ms,
        });
        true
    }

    /// The room's end stamp, once the core has fallen and the clock has been told.
    pub fn ilmari_end(&self) -> Option<&IlmariEnd> {
        self.ilmari.end.as_ref()
    }
}

fn core_event(s: &[u8; 12]) -> Option<Event> {
    (s[0] != 0).then_some(Event::Core {
        phase: s[0],
        live: s[1],
        swap_s: s[2],
        remix: s[3],
        round: s[4],
        round_s: s[5],
        tally: [s[6], s[7], s[8], s[9]],
        remember: s[10] != 0,
        stance: s[11],
    })
}
