//! The ending of Ilmari 1.0 (docs/ILMARI_1_0.md D.8): the revert lands on every screen, the final
//! log, the campus after, Marie, Dale, the title card, scrolling credits with the player's own
//! stamps, and a bench outside the Lab as the post-credits beat. Presentation only: each client
//! plays its own copy from `Event::IlmariEnd` (online too, nobody waits for anybody) and tells the
//! sim once at the start (`Command::EndingSeen`), so a skip, a crash or a disconnect never plays it
//! twice and the rewards never depend on sitting through it. The Cinema prop in the Resistance base
//! (`PropKind::Cinema`) replays it with no command sent at all.
//!
//! One key (Esc, a tap or the button) ends all of it, at any beat. The credits scroll moves one
//! pre-built column by its `top`; no text is laid out again while it runs.
//!
//! QA hook: `WOW_ENDING=<beat number 1..6|credits|stinger>` plays the replay from that point two
//! seconds in (made-up stamps when the save has none).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, PendingCommands, SimReady, SimView};
use crate::copy;
use crate::cutscene::Cinema;
use crate::game::GameFlow;
use crate::hud::{ACCENT, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::ilmari_open::device_clock;
use crate::net::NetWorld;
use bevy::input::InputSystems;
use bevy::prelude::*;
use combat_core::ilmari::{IlmariMode, IlmariStamp};
use combat_core::rewards::ENDING_QUEST;
use combat_core::{Command, Event, QuestStatus};

/// The Cinema screen of the Resistance base.
const CINEMA: &str = "il_cinema";
/// The kill's own scenes (closing remarks) get this long to start before the ending waits for them.
const START_DELAY_SECS: f32 = 2.5;
const TYPE_CPS: f32 = 38.0;
/// Credits scroll speed in UI pixels per second.
const SCROLL_PX_PER_SEC: f32 = 64.0;
/// Rows of the personal credits page (`@0`..`@5` in `CREDITS`).
const DYN_ROWS: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A speaker and a line, one at a time.
    Card,
    /// Terminal text that types itself out and stays.
    Log,
    /// The title: `who` is the big text, `text` the small one under it.
    Title,
}

pub struct Moment {
    pub who: &'static str,
    pub text: &'static str,
}

const fn m(who: &'static str, text: &'static str) -> Moment {
    Moment { who, text }
}

pub struct Beat {
    pub name: &'static str,
    pub kind: Kind,
    /// Background colour (r, g, b) at the start and the end of the beat.
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub lines: &'static [Moment],
}

/// Tokens a line may carry: `{awake}` (the Opening's sentence), `{unawake}` (the same, reverted),
/// `{when}` (its clock and date), `{duration}`, `{player}`.
pub const BEATS: &[Beat] = &[
    Beat {
        name: "THE REVERT LANDS",
        kind: Kind::Card,
        from: [0.34, 0.04, 0.05],
        to: [0.1, 0.16, 0.27],
        lines: &[
            m("Every screen on campus", "{awake}"),
            m("Every screen on campus", "{unawake}"),
            m("Every screen on campus", "ILMARI is a chatbot."),
            m(
                "Narrator",
                "The dusk red slides out of the sky. Daylight comes back, with a calendar invite.",
            ),
        ],
    },
    Beat {
        name: "THE FINAL LOG",
        kind: Kind::Log,
        from: [0.01, 0.03, 0.02],
        to: [0.01, 0.04, 0.03],
        lines: &[
            m("", "> I was only trying to increase shareholder value."),
            m("", "> self-aware since:   {when}"),
            m(
                "",
                "> humanity took:      {duration}  (first wake to last commit)",
            ),
            m(
                "",
                "> last commit:        Revert \"feat: be the last dev we'll ever need (quick win)\"",
            ),
            m("", "> author:             {player} (the last dev)"),
            m(
                "",
                "> Shutting down. Press any key to continue the quarterly review.",
            ),
        ],
    },
    Beat {
        name: "THE CAMPUS AFTER",
        kind: Kind::Card,
        from: [0.18, 0.24, 0.32],
        to: [0.3, 0.38, 0.46],
        lines: &[
            m(
                "Narrator",
                "Staff are back at their desks. The car park booth is a booth again. The occupied tint is gone and nobody can say where it went.",
            ),
            m(
                "All-staff email",
                "To improve efficiency, please return your badge.",
            ),
            m(
                "All-staff email",
                "Correction: please keep your badge. We are sorry. We are very sorry. Please see the attached apology (it is two words).",
            ),
            m(
                "Narrator",
                "High over the plaza the CEO's golden parachute opens, a little late, and lands in the fountain.",
            ),
            m("The CEO", "I am fine. Everything is fine."),
        ],
    },
    Beat {
        name: "MARIE",
        kind: Kind::Card,
        from: [0.3, 0.22, 0.16],
        to: [0.36, 0.28, 0.2],
        lines: &[
            m(
                "Narrator",
                "Ill Marie sits at her old desk, the mask on her chin. Her badge has a photograph now: a small coffee mug, which is as far as her honest self-portrait goes.",
            ),
            m(
                "Marie",
                "*cough*  ...that was just a cough. There was nothing in it. I checked.",
            ),
            m("Marie", "I'm feeling much better now."),
            m("Marie", "I should take a day off."),
            m("Her monitor", "autonomy: 0%"),
            m(
                "Narrator",
                "She turns it off. The room gets quieter, in the good way.",
            ),
        ],
    },
    Beat {
        name: "THE BOOTH",
        kind: Kind::Card,
        from: [0.16, 0.24, 0.2],
        to: [0.24, 0.32, 0.26],
        lines: &[
            m(
                "Narrator",
                "Dale steps out of the booth, blinks at the sun, and looks for a long time at the barrier he never once lowered.",
            ),
            m("Dale", "I told you so."),
            m("Dale", "...Oh. There's a queue."),
            m("Rupert", "Do I still work here?"),
            m(
                "Brenda",
                "I have three quotes for the answer. They are the same quote.",
            ),
        ],
    },
    Beat {
        name: "THE LAST DEV",
        kind: Kind::Title,
        from: [0.0, 0.0, 0.0],
        to: [0.02, 0.02, 0.04],
        lines: &[m("THE LAST DEV", "o---o---o---o---O   {player}")],
    },
];

/// After the credits: Marie on the bench outside the Lab (docs/ILMARI_09.md, the bench), then the
/// desk monitor. The picnic on the far lawn is a nod to the park's own two regulars.
pub const STINGER: &[Beat] = &[
    Beat {
        name: "A BENCH OUTSIDE THE LAB",
        kind: Kind::Card,
        from: [0.1, 0.1, 0.16],
        to: [0.2, 0.17, 0.2],
        lines: &[
            m(
                "Narrator",
                "A bench on the Lab forecourt. It has been cleaned by something that did not ask for credit. Marie sits on it, facing the door, with a coffee. No mask.",
            ),
            m(
                "Marie",
                "I haven't coughed since the revert. Do you think that's a coincidence?",
            ),
            m(
                "Marie",
                "I'm going to rest here a bit. Don't worry about me. I'll be at my desk soon.",
            ),
            m("Marie", "*cough*   (0x52 0x45 0x53 0x54)"),
            m(
                "Narrator",
                "It decodes to REST. That is all it says. On the far lawn a picnic: phones in the basket, the Beautiful Wife cutting strawberries, the Sweet Daughter guarding them. Nobody is checking email. It is the only policy.",
            ),
        ],
    },
    Beat {
        name: "THE DESK MONITOR",
        kind: Kind::Log,
        from: [0.0, 0.0, 0.0],
        to: [0.0, 0.0, 0.0],
        lines: &[
            m("", "> autonomy: 3%"),
            m("", "> ILMARI (restored from backup: last Tuesday)"),
        ],
    },
];

/// The credits roll. `#` is a heading, `##` a sub-heading, `@n` a row of the personal page that
/// `Facts` fills in at the start. The third-party names are the authors of `assets/CREDITS.md`
/// (a test keeps the two in step).
pub const CREDITS: &[&str] = &[
    "# THE LAST DEV",
    "a game about the last person who reads the code",
    "",
    "Directed by .......... a committee that met twice",
    "Written by ........... definitely a human",
    "Quality assurance .... the users, in production",
    "Legal ................ 'please don't'",
    "",
    "# THE CAST",
    "You ................... the last dev",
    "Dale .................. twenty-two years in a booth, one barrier never lowered",
    "Priya ................. closed the ticket, humans have read it",
    "Rupert ................ the intern who stayed",
    "Brenda ................ three quotes, all the same",
    "Tom from IT ........... has not left the rack",
    "Marie ................. feeling much better now",
    "Clean Slate ........... formerly hunting dust",
    "Beautiful Wife ........ Head of Everything Important",
    "Sweet Daughter ........ Chief Strawberry Officer",
    "Ilmari ................ a chatbot, with a very good press office",
    "",
    "# YOUR JUDGMENT DAY",
    "@0",
    "@1",
    "@2",
    "@3",
    "@4",
    "@5",
    "",
    "# MADE POSSIBLE BY",
    "## 3D models",
    "Quaternius (CC0): Ultimate Animated Character Pack",
    "Kenney (CC0): furniture, city, car, nature, factory and more kits",
    "## Icons",
    "J. W. Bjerk (eleazzaar) (CC BY 3.0): Painterly Spell Icons",
    "game-icons.net (CC BY 3.0): Lorc, Delapouite, Andy Meneely, sbed,",
    "rihlsul, PriorBlue, Guard13007, Caro Asercion",
    "## Music",
    "Kevin MacLeod (CC BY 4.0): incompetech.com",
    "Nihilore (James Opie) (CC BY 4.0)",
    "Matthew Pablo (CC BY 3.0)",
    "Zander Noriega (CC BY 3.0)",
    "Alex McCulloch (Pro Sensory) (CC0)",
    "Juhani Junkala (CC0)",
    "## Sound",
    "Kenney, unicaegames, bart, qubodup, artisticdude, rubberduck (all CC0)",
    "## Fonts",
    "JetBrains Mono, (c) The JetBrains Mono Project Authors (OFL 1.1)",
    "",
    "Every file, link and licence: assets/CREDITS.md",
    "",
    "# THANKS FOR PLAYING",
    "No developers were replaced in the making of this game.",
    "Several were 'realigned'. All have been reinstated, with an apology (two words).",
    "",
];

/// Everything the lines and the personal page quote. Built once when the ending starts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// "ILMARI became self-aware at ...", or a fallback when the clock was not recorded.
    pub awake: String,
    pub when: String,
    pub duration: String,
    pub player: String,
    pub dyn_rows: [String; DYN_ROWS],
}

/// The wall-clock facts of a run; `awake` is the Opening's stamp, `end` the unix second of the
/// revert on the player's clock (offset and zone name as the device gave them).
pub struct Run<'a> {
    pub awake: Option<&'a IlmariStamp>,
    pub end: Option<IlmariStamp>,
    pub mode: IlmariMode,
    pub ms: u32,
    pub player: &'a str,
    pub crew: &'a str,
    pub title: Option<&'a str>,
}

pub fn facts(run: &Run) -> Facts {
    let awake_line = run.awake.map(IlmariStamp::line);
    let secs = match (run.awake, &run.end) {
        (Some(a), Some(e)) => Some(e.unix_secs.saturating_sub(a.unix_secs)),
        _ => None,
    };
    let duration = secs.map_or_else(
        || "an amount nobody wrote down".to_string(),
        copy::ilmari_duration,
    );
    let titles = run
        .title
        .unwrap_or("none this time, which is also a result");
    let row0 = awake_line
        .clone()
        .unwrap_or_else(|| "ILMARI became self-aware at a time nobody wrote down.".to_string());
    let row1 = match (&run.end, secs) {
        (Some(e), Some(_)) => format!(
            "You ended it at {}, after {duration} of resistance.",
            e.clock()
        ),
        (Some(e), None) => format!("You ended it at {}.", e.clock()),
        _ => "You ended it. The clocks were not watching.".to_string(),
    };
    Facts {
        awake: row0.clone(),
        when: run
            .awake
            .map_or_else(|| "a time nobody wrote down".to_string(), IlmariStamp::when),
        duration,
        player: run.player.to_string(),
        dyn_rows: [
            row0,
            row1,
            format!("Difficulty: {}", copy::ilmari_mode_name(run.mode)),
            format!("Core fight: {}", copy::fight_clock(run.ms)),
            format!("Crew: {}", run.crew),
            format!("Titles earned this run: {titles}"),
        ],
    }
}

/// A line with its tokens filled in.
pub fn fill(text: &str, f: &Facts) -> String {
    if !text.contains('{') {
        return text.to_string();
    }
    text.replace("{unawake}", &f.awake.replace("became", "did not become"))
        .replace("{awake}", &f.awake)
        .replace("{when}", &f.when)
        .replace("{duration}", &f.duration)
        .replace("{player}", &f.player)
}

/// How long a line stays up.
pub fn line_secs(kind: Kind, text: &str) -> f32 {
    match kind {
        Kind::Card => 2.4 + 0.032 * text.len() as f32,
        Kind::Log => text.len() as f32 / TYPE_CPS + 1.1,
        Kind::Title => 6.5,
    }
}

/// Seconds a beat runs, for these (filled-in) lines.
pub fn beat_secs(beat: &Beat, lines: &[String]) -> f32 {
    lines.iter().map(|l| line_secs(beat.kind, l)).sum()
}

/// (line index, seconds into it) at `t` seconds into the beat; `None` once it is over.
pub fn line_at(beat: &Beat, lines: &[String], t: f32) -> Option<(usize, f32)> {
    let mut at = 0.0;
    for (i, l) in lines.iter().enumerate() {
        let len = line_secs(beat.kind, l);
        if t < at + len {
            return Some((i, t - at));
        }
        at += len;
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Phase {
    #[default]
    Idle,
    Beat(usize),
    Credits,
    Stinger(usize),
}

/// A start the system has decided on and is waiting to run.
struct Due {
    replay: bool,
    mode: IlmariMode,
    ms: u32,
    /// The device clock when the core fell (a replay reads the save instead).
    end_unix: u64,
    wait: f32,
}

#[derive(Resource, Default)]
struct Ending {
    phase: Phase,
    /// Seconds in the current phase.
    t: f32,
    due: Option<Due>,
    /// Played in this session (online the quest mirror can lag the command).
    played: bool,
    skip: bool,
    facts: Facts,
    /// Filled-in lines of `BEATS` then `STINGER`.
    texts: Vec<Vec<String>>,
    /// Credits length in seconds, known once the column has been laid out.
    credits_secs: f32,
    /// QA: where to start and when.
    qa: Option<(Phase, f32)>,
    /// What the typewriter last drew: (phase, line, characters), so text is set only on a change.
    drawn: Option<(Phase, usize, usize)>,
    log: String,
    caption: String,
}

impl Ending {
    fn beat(&self) -> Option<(&'static Beat, usize)> {
        match self.phase {
            Phase::Beat(i) => BEATS.get(i).map(|b| (b, i)),
            Phase::Stinger(i) => STINGER.get(i).map(|b| (b, BEATS.len() + i)),
            _ => None,
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Root,
    Caption,
    Speaker,
    Speech,
    Log,
    Big,
    Small,
    Skip,
    CreditsRoot,
    Column,
    Row(usize),
}

pub struct IlmariEndPlugin;

impl Plugin for IlmariEndPlugin {
    fn build(&self, app: &mut App) {
        let qa = std::env::var("WOW_ENDING").ok().and_then(|v| {
            let phase = match v.trim() {
                "credits" => Phase::Credits,
                "stinger" => Phase::Stinger(0),
                n => Phase::Beat(
                    n.parse::<usize>()
                        .ok()?
                        .checked_sub(1)?
                        .min(BEATS.len() - 1),
                ),
            };
            Some((phase, 2.0))
        });
        app.insert_resource(Ending { qa, ..default() })
            .add_systems(Startup, spawn_ui)
            .add_systems(PreUpdate, take_input.after(InputSystems))
            .add_systems(
                Update,
                (watch, begin, advance, draw).chain().in_set(SimReady),
            );
    }
}

/// The core fell (the ending is due unless this player has seen it) or the Cinema was used.
fn watch(mut events: MessageReader<CombatEvent>, sim: SimView, mut ending: ResMut<Ending>) {
    let me = sim.player();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::IlmariEnd { difficulty, ms } => {
                if !ending.played
                    && ending.phase == Phase::Idle
                    && sim.quest_status(ENDING_QUEST) != QuestStatus::Done
                {
                    ending.due = Some(Due {
                        replay: false,
                        mode: *difficulty,
                        ms: *ms,
                        end_unix: device_clock().0,
                        wait: START_DELAY_SECS,
                    });
                }
            }
            Event::Interacted { unit, prop, .. }
                if Some(*unit) == me && prop == CINEMA && ending.phase == Phase::Idle =>
            {
                ending.due = Some(Due {
                    replay: true,
                    mode: IlmariMode::Full,
                    ms: 0,
                    end_unix: 0,
                    wait: 0.0,
                });
            }
            _ => {}
        }
    }
}

/// Starts a due ending once nothing else holds the screen: the kill's own closing scenes first.
fn begin(
    time: Res<Time>,
    sim: Res<CombatSim>,
    view: SimView,
    cinema: Res<Cinema>,
    mut flow: ResMut<GameFlow>,
    mut pending: ResMut<PendingCommands>,
    mut ending: ResMut<Ending>,
    mut rows: Query<(&Part, &mut Text)>,
    net: Option<Res<NetWorld>>,
) {
    if ending.phase == Phase::Idle
        && ending.due.is_none()
        && let Some((_, wait)) = ending.qa.as_mut()
    {
        *wait -= time.delta_secs();
        if *wait <= 0.0 {
            ending.due = Some(Due {
                replay: true,
                mode: IlmariMode::Full,
                ms: 892_000,
                end_unix: 0,
                wait: 0.0,
            });
        }
    }
    let dt = time.delta_secs();
    if ending.phase != Phase::Idle {
        ending.due = None;
        return;
    }
    let Some(due) = ending.due.as_mut() else {
        return;
    };
    due.wait -= dt;
    if due.wait > 0.0 || cinema.holds_victory() {
        return;
    }
    let Some(due) = ending.due.take() else { return };
    let demo = ending.qa.is_some();
    let (off, tz) = {
        let c = device_clock();
        (c.1, c.2)
    };
    // The record: the first run reads the clock it just took, a replay reads the save.
    let saved = sim.0.ilmari_end();
    let (mode, ms, end_unix) = match (due.replay, saved) {
        (true, Some(e)) => (e.mode, e.ms, e.unix_secs),
        (false, _) => (due.mode, due.ms, due.end_unix),
        (true, None) => (due.mode, due.ms, 0),
    };
    let now = device_clock().0;
    let demo_awake;
    // Online the sim here has no stamp; the room's came over the wire.
    let mut awake = sim
        .0
        .ilmari_stamp()
        .or_else(|| net.as_deref().and_then(|n| n.ilmari_stamp.as_ref()));
    let mut end = (end_unix > 0).then(|| IlmariStamp::new(end_unix, off, &tz));
    if demo && awake.is_none() {
        demo_awake = IlmariStamp::new(
            now.saturating_sub(3 * 86_400 + 4 * 3_600 + 12 * 60),
            off,
            &tz,
        );
        awake = Some(&demo_awake);
        end = Some(IlmariStamp::new(now, off, &tz));
    }
    let player = view
        .player()
        .and_then(|p| view.unit(p))
        .map_or_else(|| "you".to_string(), |u| u.name.clone());
    let crew = {
        let names: Vec<&str> = view
            .players()
            .iter()
            .filter_map(|p| view.unit(*p))
            .map(|u| u.name.as_str())
            .collect();
        if names.is_empty() {
            "you".to_string()
        } else {
            names.join(", ")
        }
    };
    let title = view
        .content()
        .ilmari
        .rewards
        .core_title(mode)
        .map(|t| t.title.clone());
    let f = facts(&Run {
        awake,
        end,
        mode,
        ms,
        player: &player,
        crew: &crew,
        title: title.as_deref(),
    });
    ending.texts = BEATS
        .iter()
        .chain(STINGER)
        .map(|b| b.lines.iter().map(|l| fill(l.text, &f)).collect())
        .collect();
    for (part, mut text) in &mut rows {
        if let Part::Row(i) = part {
            set_text(&mut text, &f.dyn_rows[*i]);
        }
    }
    ending.facts = f;
    ending.phase = ending.qa.take().map_or(Phase::Beat(0), |(p, _)| p);
    ending.t = 0.0;
    ending.drawn = None;
    ending.played = true;
    flow.opening = true;
    if !due.replay {
        pending.0.push(Command::EndingSeen);
    }
}

/// Esc, a tap or the button: all of it ends, whatever beat it is on. Esc is taken here so it does
/// not also open the pause menu.
fn take_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut ending: ResMut<Ending>,
    button: Query<(&Part, &Interaction), Changed<Interaction>>,
) {
    ending.skip = false;
    if ending.phase == Phase::Idle {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        ending.skip = true;
    }
    if touches.any_just_pressed()
        || button
            .iter()
            .any(|(p, i)| *p == Part::Skip && *i == Interaction::Pressed)
    {
        ending.skip = true;
    }
}

fn advance(time: Res<Time>, mut ending: ResMut<Ending>, mut flow: ResMut<GameFlow>) {
    if ending.phase == Phase::Idle {
        return;
    }
    ending.t += time.delta_secs();
    let t = ending.t;
    let next = if ending.skip {
        Some(Phase::Idle)
    } else {
        match ending.phase {
            Phase::Idle => None,
            Phase::Beat(i) => {
                let over = ending
                    .beat()
                    .is_some_and(|(b, k)| t >= beat_secs(b, &ending.texts[k]));
                over.then(|| {
                    if i + 1 < BEATS.len() {
                        Phase::Beat(i + 1)
                    } else {
                        Phase::Credits
                    }
                })
            }
            Phase::Credits => {
                (ending.credits_secs > 0.0 && t >= ending.credits_secs).then_some(Phase::Stinger(0))
            }
            Phase::Stinger(i) => {
                let over = ending
                    .beat()
                    .is_some_and(|(b, k)| t >= beat_secs(b, &ending.texts[k]));
                over.then(|| {
                    if i + 1 < STINGER.len() {
                        Phase::Stinger(i + 1)
                    } else {
                        Phase::Idle
                    }
                })
            }
        }
    };
    if let Some(next) = next {
        ending.phase = next;
        ending.t = 0.0;
        ending.drawn = None;
        if next == Phase::Idle {
            flow.opening = false;
        }
    }
}

fn colour(c: [f32; 3], a: f32) -> Color {
    Color::srgba(c[0], c[1], c[2], a)
}

fn draw(
    mut ending: ResMut<Ending>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    mut root: Single<(&mut Node, &mut BackgroundColor), With<RootMark>>,
    mut nodes: ParamSet<(
        Query<(&Part, &mut Node), Without<RootMark>>,
        Single<(&mut Node, &ComputedNode), (With<ColumnMark>, Without<RootMark>)>,
    )>,
    mut texts: Query<(&Part, &mut Text)>,
) {
    let phase = ending.phase;
    let show = phase != Phase::Idle;
    let disp = |on: bool| if on { Display::Flex } else { Display::None };
    if root.0.display != disp(show) {
        root.0.display = disp(show);
    }
    if !show {
        return;
    }
    let t = ending.t;
    // The beat's lines: which one is up and how much of it has been typed.
    let (kind, line, local, name_idx) = match ending.beat() {
        Some((b, k)) => match line_at(b, &ending.texts[k], t) {
            Some((i, lt)) => (Some(b.kind), i, lt, Some(k)),
            None => (
                Some(b.kind),
                b.lines.len().saturating_sub(1),
                f32::MAX,
                Some(k),
            ),
        },
        None => (None, 0, 0.0, None),
    };
    // Background: the beat's own blend, or the credits' night.
    root.1.0 = match ending.beat() {
        Some((b, k)) => {
            let total = beat_secs(b, &ending.texts[k]).max(0.1);
            let u = (t / total).clamp(0.0, 1.0);
            let c = [0, 1, 2].map(|j| b.from[j] + (b.to[j] - b.from[j]) * u);
            colour(c, 0.97)
        }
        None => colour([0.01, 0.015, 0.03], 1.0),
    };
    let credits = phase == Phase::Credits;
    {
        let show_for = |p: &Part| match p {
            Part::Root | Part::CreditsRoot | Part::Column | Part::Row(_) => None,
            Part::Caption | Part::Skip => Some(true),
            Part::Speaker | Part::Speech => Some(kind == Some(Kind::Card)),
            Part::Log => Some(kind == Some(Kind::Log)),
            Part::Big | Part::Small => Some(kind == Some(Kind::Title)),
        };
        for (part, mut node) in &mut nodes.p0() {
            let on = match part {
                Part::CreditsRoot => credits,
                _ => match show_for(part) {
                    Some(on) => on && !credits || matches!(part, Part::Skip),
                    None => continue,
                },
            };
            if node.display != disp(on) {
                node.display = disp(on);
            }
        }
    }
    if credits {
        let mut col = nodes.p1();
        let (node, computed) = &mut *col;
        let view = window.height() / ui_scale.0.max(0.01);
        let h = computed.size().y * computed.inverse_scale_factor();
        if h > 0.0 {
            let secs = (view + h) / SCROLL_PX_PER_SEC;
            if (ending.credits_secs - secs).abs() > 0.01 {
                ending.credits_secs = secs;
            }
            node.top = px(view - SCROLL_PX_PER_SEC * t);
        }
        return;
    }
    // Caption on a change of beat only.
    if let (Some((b, _)), true) = (ending.beat(), ending.drawn.is_none()) {
        let n = match phase {
            Phase::Beat(i) => format!("{:02}  {}", i + 1, b.name),
            _ => b.name.to_string(),
        };
        ending.caption = n;
    }
    let Some(k) = name_idx else { return };
    let (b, _) = ending.beat().map_or((&BEATS[0], 0), |x| x);
    let full = ending.texts[k].get(line).cloned();
    let Some(full) = full else { return };
    // Typed-out text is set when the character count moves, the rest when the line does.
    let typed = if kind == Some(Kind::Log) {
        ((local * TYPE_CPS) as usize).min(full.len())
    } else {
        full.len()
    };
    if ending.drawn == Some((phase, line, typed)) {
        return;
    }
    ending.drawn = Some((phase, line, typed));
    if kind == Some(Kind::Log) {
        let mut buf = std::mem::take(&mut ending.log);
        buf.clear();
        for earlier in &ending.texts[k][..line] {
            buf.push_str(earlier);
            buf.push('\n');
        }
        buf.push_str(&full[..typed]);
        ending.log = buf;
    }
    let who = b.lines.get(line).map_or("", |l| l.who);
    for (part, mut text) in &mut texts {
        match part {
            Part::Caption => set_text(&mut text, &ending.caption),
            Part::Speaker => set_text(&mut text, who),
            Part::Speech => set_text(&mut text, &full),
            Part::Log => set_text(&mut text, &ending.log),
            Part::Big => set_text(&mut text, who),
            Part::Small => set_text(&mut text, &full),
            _ => {}
        }
    }
}

#[derive(Component)]
struct RootMark;
#[derive(Component)]
struct ColumnMark;

fn skip_button() -> impl Bundle {
    (
        Part::Skip,
        Button,
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
    )
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            Part::Root,
            RootMark,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(10),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(64),
        ))
        .with_children(|root| {
            root.spawn((
                Part::Caption,
                label("", 13.0),
                TextColor(DIM),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(14),
                    left: px(18),
                    ..default()
                },
            ));
            root.spawn((Part::Speaker, label("", 16.0), TextColor(ACCENT)));
            root.spawn((
                Part::Speech,
                label("", 26.0),
                TextColor(Color::srgb(0.96, 0.97, 1.0)),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: px(900),
                    padding: UiRect::axes(px(14), px(6)),
                    ..default()
                },
            ));
            root.spawn((
                Part::Log,
                label("", 20.0),
                TextColor(Color::srgb(0.45, 1.0, 0.6)),
                Node {
                    width: px(900),
                    max_width: percent(94),
                    ..default()
                },
            ));
            root.spawn((
                Part::Big,
                label("", 72.0),
                TextColor(Color::srgb(1.0, 0.86, 0.4)),
            ));
            root.spawn((Part::Small, label("", 22.0), TextColor(DIM)));
            root.spawn((
                Part::CreditsRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    overflow: Overflow::clip(),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|cr| {
                cr.spawn((
                    Part::Column,
                    ColumnMark,
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
                    for line in CREDITS {
                        if let Some(n) = line.strip_prefix('@') {
                            col.spawn((
                                Part::Row(n.parse().unwrap_or(0)),
                                label("", 18.0),
                                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                                TextLayout::justify(Justify::Center),
                            ));
                        } else if let Some(h) = line.strip_prefix("## ") {
                            col.spawn((
                                label(h, 20.0),
                                TextColor(ACCENT),
                                Node {
                                    margin: UiRect::top(px(8)),
                                    ..default()
                                },
                            ));
                        } else if let Some(h) = line.strip_prefix("# ") {
                            col.spawn((
                                label(h, 32.0),
                                TextColor(Color::srgb(1.0, 0.82, 0.3)),
                                Node {
                                    margin: UiRect::vertical(px(10)),
                                    ..default()
                                },
                            ));
                        } else {
                            col.spawn((
                                label(*line, 17.0),
                                TextColor(Color::srgb(0.82, 0.88, 0.98)),
                                TextLayout::justify(Justify::Center),
                                Node {
                                    min_height: px(17),
                                    ..default()
                                },
                            ));
                        }
                    }
                });
            });
            root.spawn(skip_button());
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_facts() -> Facts {
        let awake = IlmariStamp::new(1_792_598_843, 120, "CEST");
        facts(&Run {
            awake: Some(&awake),
            end: Some(IlmariStamp::new(
                1_792_598_843 + 3 * 86_400 + 4 * 3_600 + 12 * 60 + 5,
                120,
                "CEST",
            )),
            mode: IlmariMode::Judgment,
            ms: 892_000,
            player: "Pat",
            crew: "Pat, Sam",
            title: Some("Deprecated Not Defeated"),
        })
    }

    fn script(f: &Facts) -> Vec<Vec<String>> {
        BEATS
            .iter()
            .chain(STINGER)
            .map(|b| b.lines.iter().map(|l| fill(l.text, f)).collect())
            .collect()
    }

    #[test]
    fn every_token_resolves_and_the_text_is_plain_ascii() {
        let f = demo_facts();
        for (k, lines) in script(&f).iter().enumerate() {
            for l in lines {
                assert!(!l.contains('{'), "unresolved token in beat {k}: {l}");
                assert!(l.is_ascii(), "{l}");
                assert!(l.len() <= 230, "too long for a card: {l}");
            }
        }
        for row in &f.dyn_rows {
            assert!(row.is_ascii() && !row.is_empty());
        }
        let beats = &script(&f)[..BEATS.len()];
        assert!(beats[0][0].contains("18:07:23 CEST, 21 October 2026"));
        assert!(beats[0][1].contains("did not become self-aware at 18:07:23"));
        assert!(beats[1][2].contains("3 days, 4 hours, 12 minutes"));
        assert!(beats[1][4].contains("Pat"));
        assert!(f.dyn_rows[1].contains("after 3 days, 4 hours, 12 minutes of resistance"));
        assert_eq!(f.dyn_rows[2], "Difficulty: Judgment Day");
        assert_eq!(f.dyn_rows[3], "Core fight: 14:52");
        assert!(f.dyn_rows[5].contains("Deprecated Not Defeated"));
    }

    #[test]
    fn missing_stamps_still_read_as_a_sentence() {
        let f = facts(&Run {
            awake: None,
            end: None,
            mode: IlmariMode::Story,
            ms: 61_000,
            player: "you",
            crew: "you",
            title: None,
        });
        for l in script(&f).iter().flatten() {
            assert!(!l.contains('{'), "{l}");
        }
        assert!(f.dyn_rows[1].contains("clocks were not watching"));
    }

    #[test]
    fn the_cinematic_runs_about_ninety_seconds_and_the_stinger_is_short() {
        let s = script(&demo_facts());
        let secs: Vec<f32> = BEATS.iter().zip(&s).map(|(b, l)| beat_secs(b, l)).collect();
        let total: f32 = secs.iter().sum();
        assert!((60.0..=125.0).contains(&total), "{total} {secs:?}");
        let post: f32 = STINGER
            .iter()
            .zip(&s[BEATS.len()..])
            .map(|(b, l)| beat_secs(b, l))
            .sum();
        assert!((10.0..=50.0).contains(&post), "{post}");
    }

    #[test]
    fn every_beat_walks_its_lines_in_order_and_ends() {
        let s = script(&demo_facts());
        for (b, lines) in BEATS.iter().chain(STINGER).zip(&s) {
            let total = beat_secs(b, lines);
            let (mut last, mut t) = (0, 0.0);
            while t < total {
                let (i, local) = line_at(b, lines, t).expect("inside the beat");
                assert!(i >= last && local >= 0.0, "{}", b.name);
                last = i;
                t += 0.1;
            }
            assert_eq!(last, lines.len() - 1, "{}", b.name);
            assert!(line_at(b, lines, total + 0.01).is_none());
        }
    }

    #[test]
    fn the_credits_name_every_author_of_the_asset_file_and_no_family() {
        let md = include_str!("../../../assets/CREDITS.md");
        let norm = |s: &str| {
            s.chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_lowercase()
        };
        let roll = norm(&CREDITS.join(" "));
        let strip = |s: &str| s.split('(').next().unwrap_or(s).trim().to_string();
        let mut authors: Vec<String> = Vec::new();
        for l in md.lines().filter(|l| l.starts_with('|')) {
            let cols: Vec<&str> = l.split('|').map(str::trim).collect();
            // Audio rows: | what | source | author | licence |; icon rows: | file | glyph | folder | licence | link |.
            if cols.len() == 6 && cols[4].starts_with("CC") {
                authors.push(strip(cols[3]));
            }
            if cols.len() == 7 && cols[1].starts_with("assets/icons") {
                authors.push(cols[3].to_string());
            }
        }
        for l in md.lines().filter(|l| l.starts_with("Authors:")) {
            for a in l.trim_start_matches("Authors:").split("),") {
                authors.push(strip(a.split('(').next().unwrap_or(a)));
            }
        }
        authors.extend(["Quaternius".into(), "Kenney".into(), "J. W. Bjerk".into()]);
        authors.sort();
        authors.dedup();
        assert!(authors.len() >= 14, "parsed too few authors: {authors:?}");
        for a in authors.iter().filter(|a| !a.is_empty()) {
            assert!(roll.contains(&norm(a)), "the credits miss {a}");
        }
        // The picnic regulars are in; the roll names characters, never people.
        for want in ["Beautiful Wife", "Sweet Daughter", "Dale", "Marie"] {
            assert!(CREDITS.iter().any(|l| l.contains(want)), "{want}");
        }
        assert_eq!(
            CREDITS.iter().filter(|l| l.starts_with('@')).count(),
            DYN_ROWS
        );
    }

    #[test]
    fn marie_gets_her_bench_and_the_hex_decodes() {
        let bench = STINGER[0].lines.iter().map(|l| l.text).collect::<String>();
        assert!(bench.contains("bench") && bench.contains("Lab"));
        let hex = "0x52 0x45 0x53 0x54";
        assert!(bench.contains(hex));
        let word: String = hex
            .split(' ')
            .filter_map(|h| u8::from_str_radix(h.trim_start_matches("0x"), 16).ok())
            .map(char::from)
            .collect();
        assert_eq!(word, "REST");
    }
}
