//! Short in-engine cutscenes after the campaign bosses (letterbox bars, a slow camera move to the
//! fallen boss, two to four subtitled lines) and the epilogue after the AGI reveal, which ends in
//! the credits roll. They are presentation only: the sim keeps running (online the server never
//! waits), Esc, a tap or the skip button ends one, and sprint runs never play them.
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

pub const SCENES: &[Scene] = &[
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
        cinema.playing = None;
        if SCENES[i].boss == EPILOGUE {
            cinema.credits = Some(0.0);
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
        for s in SCENES {
            assert!(
                sim.content().unit_def(s.boss).is_some(),
                "{} is unknown",
                s.boss
            );
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
