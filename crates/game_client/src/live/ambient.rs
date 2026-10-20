//! Campus extras: people and robots with a daily routine, so the open world looks lived in.
//! Commuters stream from the station and the South Gate to the tower in the morning and back in
//! the evening, joggers circle the lake, smokers chat by the tower door, people on calls pace,
//! delivery robots work the boulevard, and a guard walks the ring road at night. Each says a
//! line of its district when you pass.
//!
//! Pure presentation: no sim units, nothing on the wire. Positions come from the office clock
//! (`combat_core::live`, one game day = 24 real minutes) and app time, so every client shows the
//! same streets without syncing anything. Extras beyond `SHOW_M` from the player are hidden.

use super::{LiveState, unix_now};
use crate::arena::Ground;
use crate::catalog::IdlePose;
use crate::combat::{CombatSim, SimView};
use crate::game::GameFlow;
use crate::net::NetWorld;
use crate::player::Player;
use crate::yell::AmbientSay;
use bevy::prelude::*;
use combat_core::live::game_minute;

/// Extras further than this are not drawn (skinned meshes are the expensive part on WebGL2).
const SHOW_M: f32 = 70.0;
/// A passing player hears an extra within this range...
const SAY_M: f32 = 7.0;
/// ...at most this often from one extra, and this often overall.
const SAY_EACH_S: f32 = 45.0;
const SAY_ANY_S: f32 = 10.0;
/// Game minute used when the office clock is off (QA shots): mid-morning, everyone about.
const NO_CLOCK_MIN: u32 = 8 * 60 + 30;

#[derive(Clone, Copy, PartialEq)]
enum Routine {
    /// Round and round a closed route.
    Loop,
    /// Along the route in the morning window, back along it in the evening one.
    Commute,
    /// Back and forth between the route's ends.
    Pace,
    /// Standing at the first point, chatting.
    Idle,
}

struct Extra {
    key: &'static str,
    route: &'static [(f32, f32)],
    routine: Routine,
    speed: f32,
    /// Game minutes [from, to) the extra is about; wraps past midnight.
    hours: (u32, u32),
    /// Seconds of head start along the route, so a group spreads out.
    phase: f32,
    lines: &'static [&'static str],
}

const STATION_TO_TOWER: &[(f32, f32)] = &[
    (150.0, -146.0),
    (150.0, -110.0),
    (2.0, -110.0),
    (2.0, -54.0),
    (2.0, -27.0),
];
const GATE_TO_TOWER: &[(f32, f32)] = &[(-2.5, 157.0), (-2.5, 55.0), (-2.0, -27.0)];
const LAKE_LOOP: &[(f32, f32)] = &[
    (-124.0, -5.0),
    (-131.6, -23.4),
    (-150.0, -31.0),
    (-168.4, -23.4),
    (-176.0, -5.0),
    (-168.4, 13.4),
    (-150.0, 21.0),
    (-131.6, 13.4),
    (-124.0, -5.0),
];
const BOULEVARD_LOOP: &[(f32, f32)] = &[
    (3.0, 58.0),
    (3.0, 150.0),
    (-3.0, 150.0),
    (-3.0, 58.0),
    (3.0, 58.0),
];
const RING_ROAD: &[(f32, f32)] = &[
    (-71.0, 52.0),
    (71.0, 52.0),
    (71.0, -50.0),
    (-71.0, -50.0),
    (-71.0, 52.0),
];
const INCUBATOR_LOOP: &[(f32, f32)] = &[
    (115.0, -44.0),
    (115.0, 46.0),
    (119.0, 46.0),
    (119.0, -44.0),
    (115.0, -44.0),
];

const COMMUTER_LINES: &[&str] = &[
    "Ninety minutes on the train to badge in and join a Teams call.",
    "If I swipe my badge twice, does it count as two office days?",
    "The 08:12 was cancelled, so I'm early for the 09:00. Which was also cancelled.",
    "I parked at P3. I've been walking since Tuesday.",
];
const JOGGER_LINES: &[&str] = &[
    "Nine kilometres before standup. Ask me about my resting heart rate.",
    "On your left! Also on your calendar.",
    "My watch says I'm thriving. My manager says I'm 'present'.",
    "The goose is out today. Run clockwise. Never anticlockwise.",
];
const SMOKER_LINES: &[&str] = &[
    "I'm not smoking. I'm stepping out for alignment.",
    "Did you see the reorg email? Me neither. I'm in it.",
    "My manager booked a meeting about having fewer meetings.",
    "The new HQ opens in 2019. I've stopped asking which 2019.",
];
const CALL_LINES: &[&str] = &[
    "Can you hear me? Can you see my screen? Is that a yes?",
    "Let's take this offline. We are offline. Let's take it online.",
    "Sorry, you were on mute. For the whole quarter.",
    "Per my last email. And the one before. And the voicemail.",
];
const FOUNDER_CALL_LINES: &[&str] = &[
    "We're pre-revenue but post-vibes. Can I send you the deck?",
    "It's Uber for Ubers. No, the other way round.",
    "Our burn rate is a feature. It keeps us warm.",
];
const ROBOT_LINES: &[&str] = &[
    "Delivering: one oat latte to Desk 0. Desk 0 does not exist.",
    "Excuse me. Excuse me. Recalculating. Excuse you.",
    "Package for the AI Agent. Signature required. The AI Agent signs everything.",
];
const SCOOTER_LINES: &[&str] = &[
    "Disrupting the pavement! Thought leadership on wheels!",
    "I'm late for my keynote about being early.",
];
const GUARD_LINES: &[&str] = &[
    "Campus is closed. The AI is still in the building. It's always in the building.",
    "Night shift. The badge readers sleep, I don't.",
];

const EXTRAS: [Extra; 22] = [
    // Morning and evening commuters from the station...
    commuter("rto_commuter", STATION_TO_TOWER, 0.0),
    commuter("middle_manager", STATION_TO_TOWER, 30.0),
    commuter("scrum_master", STATION_TO_TOWER, 65.0),
    commuter("product_owner", STATION_TO_TOWER, 100.0),
    commuter("hybrid_worker", STATION_TO_TOWER, 140.0),
    // ...and the ones who parked at P3 and walked in through the South Gate.
    commuter("meatproxy", GATE_TO_TOWER, 10.0),
    commuter("vibe_junior", GATE_TO_TOWER, 50.0),
    commuter("growth_hacker", GATE_TO_TOWER, 95.0),
    // Joggers round the lake, day only.
    jogger("wellness_manager", 0.0),
    jogger("agile_coach", 40.0),
    jogger("step_counter", 80.0),
    // Smokers by the tower door, working hours.
    idle("middle_manager", &[(-7.0, -21.0)]),
    idle("sales_rep", &[(-5.5, -22.4)]),
    // People on calls, pacing.
    Extra {
        key: "account_manager",
        route: &[(-12.0, -13.0), (-3.0, -13.0)],
        routine: Routine::Pace,
        speed: 1.1,
        hours: (9 * 60, 18 * 60),
        phase: 0.0,
        lines: CALL_LINES,
    },
    Extra {
        key: "consultant",
        route: &[(14.0, 62.0), (34.0, 62.0)],
        routine: Routine::Pace,
        speed: 1.0,
        hours: (9 * 60, 18 * 60),
        phase: 3.0,
        lines: CALL_LINES,
    },
    Extra {
        key: "preseed_founder",
        route: &[(111.0, 22.0), (111.0, 40.0)],
        routine: Routine::Pace,
        speed: 1.2,
        hours: (8 * 60, 23 * 60),
        phase: 0.0,
        lines: FOUNDER_CALL_LINES,
    },
    // Delivery robots never stop.
    robot(BOULEVARD_LOOP, 0.0),
    robot(BOULEVARD_LOOP, 90.0),
    robot(INCUBATOR_LOOP, 20.0),
    Extra {
        key: "scooter_exec",
        route: BOULEVARD_LOOP,
        routine: Routine::Loop,
        speed: 5.5,
        hours: (8 * 60, 19 * 60),
        phase: 15.0,
        lines: SCOOTER_LINES,
    },
    // The night guard walks the ring road.
    Extra {
        key: "security_analyst",
        route: RING_ROAD,
        routine: Routine::Loop,
        speed: 1.3,
        hours: (19 * 60, 7 * 60),
        phase: 0.0,
        lines: GUARD_LINES,
    },
    Extra {
        key: "security_analyst",
        route: RING_ROAD,
        routine: Routine::Loop,
        speed: 1.3,
        hours: (19 * 60, 7 * 60),
        phase: 220.0,
        lines: GUARD_LINES,
    },
];

const fn commuter(key: &'static str, route: &'static [(f32, f32)], phase: f32) -> Extra {
    Extra {
        key,
        route,
        routine: Routine::Commute,
        speed: 1.4,
        hours: (0, 24 * 60),
        phase,
        lines: COMMUTER_LINES,
    }
}

const fn jogger(key: &'static str, phase: f32) -> Extra {
    Extra {
        key,
        route: LAKE_LOOP,
        routine: Routine::Loop,
        speed: 3.2,
        hours: (6 * 60, 21 * 60),
        phase,
        lines: JOGGER_LINES,
    }
}

const fn idle(key: &'static str, at: &'static [(f32, f32)]) -> Extra {
    Extra {
        key,
        route: at,
        routine: Routine::Idle,
        speed: 0.0,
        hours: (8 * 60, 19 * 60),
        phase: 0.0,
        lines: SMOKER_LINES,
    }
}

const fn robot(route: &'static [(f32, f32)], phase: f32) -> Extra {
    Extra {
        key: "delivery_robot",
        route,
        routine: Routine::Loop,
        speed: 1.8,
        hours: (0, 24 * 60),
        phase,
        lines: ROBOT_LINES,
    }
}

/// Morning and evening commute windows (game minutes) for `Routine::Commute`.
const MORNING: (u32, u32) = (7 * 60, 10 * 60);
const EVENING: (u32, u32) = (16 * 60 + 30, 19 * 60 + 30);

fn in_hours(m: u32, (from, to): (u32, u32)) -> bool {
    if from <= to {
        (from..to).contains(&m)
    } else {
        m >= from || m < to
    }
}

fn route_len(r: &[(f32, f32)]) -> f32 {
    r.windows(2)
        .map(|w| Vec2::new(w[1].0 - w[0].0, w[1].1 - w[0].1).length())
        .sum()
}

/// Point and heading at distance `d` along the route (clamped to its ends).
fn along(r: &[(f32, f32)], d: f32) -> (Vec2, Vec2) {
    let mut left = d.max(0.0);
    for w in r.windows(2) {
        let (a, b) = (Vec2::new(w[0].0, w[0].1), Vec2::new(w[1].0, w[1].1));
        let seg = a.distance(b);
        if left <= seg {
            let dir = (b - a).normalize_or_zero();
            return (a + dir * left, dir);
        }
        left -= seg;
    }
    let n = r.len();
    let end = Vec2::new(r[n - 1].0, r[n - 1].1);
    let prev = if n > 1 {
        Vec2::new(r[n - 2].0, r[n - 2].1)
    } else {
        end
    };
    (end, (end - prev).normalize_or_zero())
}

/// Where an extra is at office time `clock` (game minutes, fractional) and app time `t`
/// (seconds), facing which way; `None` while it is off duty or not on its way.
fn place(e: &Extra, clock: f32, t: f32) -> Option<(Vec2, Vec2)> {
    let minute = clock as u32 % (24 * 60);
    if !in_hours(minute, e.hours) {
        return None;
    }
    let len = route_len(e.route);
    match e.routine {
        Routine::Idle => Some((Vec2::new(e.route[0].0, e.route[0].1), Vec2::new(0.3, 1.0))),
        Routine::Loop if len > 0.0 => Some(along(
            e.route,
            (t * e.speed + e.phase * e.speed).rem_euclid(len),
        )),
        Routine::Pace if len > 0.0 => {
            let d = (t * e.speed + e.phase).rem_euclid(2.0 * len);
            let (p, dir) = along(e.route, if d < len { d } else { 2.0 * len - d });
            Some((p, if d < len { dir } else { -dir }))
        }
        Routine::Commute => {
            // One game minute is one real second: the window's elapsed game minutes are the
            // seconds walked since it opened.
            let (window, back) = if in_hours(minute, MORNING) {
                (MORNING, false)
            } else if in_hours(minute, EVENING) {
                (EVENING, true)
            } else {
                return None;
            };
            let walked = clock.rem_euclid(24.0 * 60.0) - window.0 as f32 - e.phase;
            let d = walked * e.speed;
            if d < 0.0 || d > len {
                return None;
            }
            let (p, dir) = along(e.route, if back { len - d } else { d });
            Some((p, if back { -dir } else { dir }))
        }
        _ => None,
    }
}

#[derive(Component)]
struct ExtraUnit {
    index: usize,
    said_at: f32,
}

#[derive(Resource, Default)]
struct ExtrasState {
    zone: String,
    said_at: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ExtrasState>().add_systems(
        Update,
        drive_extras
            .after(crate::game::FloorSwap)
            .in_set(crate::combat::SimReady),
    );
}

fn drive_extras(
    mut commands: Commands,
    time: Res<Time>,
    flow: Res<GameFlow>,
    ground: Res<Ground>,
    (sim, offline, net, live): (
        SimView,
        Res<CombatSim>,
        Option<Res<NetWorld>>,
        Res<LiveState>,
    ),
    mut state: ResMut<ExtrasState>,
    player: Query<&Transform, (With<Player>, Without<ExtraUnit>)>,
    mut extras: Query<(Entity, &mut ExtraUnit, &mut Transform, &mut Visibility)>,
    mut say: MessageWriter<AmbientSay>,
) {
    let campus = flow.zone == "campus";
    if state.zone != flow.zone {
        state.zone.clone_from(&flow.zone);
        for (e, ..) in &extras {
            commands.entity(e).despawn();
        }
        if campus {
            for (index, x) in EXTRAS.iter().enumerate() {
                let pos = Vec3::new(x.route[0].0, 0.0, x.route[0].1);
                let pose = if x.routine == Routine::Idle {
                    IdlePose::Chatting
                } else {
                    IdlePose::Standing
                };
                let e = crate::units::spawn_visual(&mut commands, x.key, pos, 0.0, pose);
                commands.entity(e).insert((
                    ExtraUnit {
                        index,
                        said_at: -SAY_EACH_S,
                    },
                    Visibility::Hidden,
                ));
            }
        }
        return;
    }
    if !campus {
        return;
    }
    let t = time.elapsed_secs();
    // Without the office clock the morning commute loops, so QA shots still show the streets.
    let clock = unix_now(&*sim, &offline, net.is_some(), &live)
        .map_or(NO_CLOCK_MIN as f32 + t.rem_euclid(150.0), |u| {
            game_minute(u as u64) as f32 + u.fract() as f32
        });
    let me = player.single().map(|p| p.translation).ok();
    for (e, mut x, mut tf, mut vis) in &mut extras {
        let def = &EXTRAS[x.index];
        let spot = place(def, clock, t);
        let near = |p: Vec2| me.is_some_and(|m| Vec2::new(m.x, m.z).distance(p) < SHOW_M);
        let Some((p, dir)) = spot.filter(|(p, _)| near(*p)) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let pos = Vec3::new(p.x, ground.0.height(p.x, p.y), p.y);
        tf.translation = pos;
        if dir.length_squared() > 0.0 {
            tf.rotation = Quat::from_rotation_y((-dir.x).atan2(-dir.y));
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let close = me.is_some_and(|m| m.distance(pos) < SAY_M);
        if close && t - x.said_at > SAY_EACH_S && t - state.said_at > SAY_ANY_S {
            let line = def.lines[(t as usize + x.index) % def.lines.len()];
            x.said_at = t;
            state.said_at = t;
            say.write(AmbientSay {
                anchor: e,
                head: crate::config::head_height(def.key),
                text: line.to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routines_follow_the_office_clock() {
        let commuter = &EXTRAS[0];
        // Off duty at night, on the way in the morning, gone home by 21:00.
        assert!(place(commuter, 180.0, 0.0).is_none());
        let Some((p, _)) = place(commuter, (MORNING.0 + 20) as f32, 0.0) else {
            panic!("no morning commute")
        };
        assert!(p.distance(Vec2::new(150.0, -146.0)) < 40.0, "{p}");
        assert!(place(commuter, 1260.0, 0.0).is_none());
        // The evening leg runs the route backwards: it starts at the tower.
        let Some((q, _)) = place(commuter, (EVENING.0 + 2) as f32, 0.0) else {
            panic!("no evening commute")
        };
        assert!(q.distance(Vec2::new(2.0, -27.0)) < 10.0, "{q}");
        // Joggers keep to the loop and stay home at night; the guard only works nights.
        let jogger = &EXTRAS[8];
        let Some((j, _)) = place(jogger, 720.0, 37.0) else {
            panic!("no jogger")
        };
        assert!((j.distance(Vec2::new(-150.0, -5.0)) - 26.0).abs() < 3.0);
        assert!(place(jogger, 1380.0, 0.0).is_none());
        let guard = &EXTRAS[20];
        assert!(place(guard, 720.0, 0.0).is_none() && place(guard, 120.0, 0.0).is_some());
        // Pacing turns round at both ends.
        let pacer = &EXTRAS[13];
        for t in 0..60 {
            let Some((p, _)) = place(pacer, 600.0, t as f32) else {
                panic!("pacer off duty")
            };
            assert!(p.x >= -12.01 && p.x <= -2.99, "{p}");
        }
    }

    #[test]
    fn every_extra_has_a_known_look_and_lines() {
        for e in &EXTRAS {
            assert!(crate::catalog::is_known(e.key), "{}", e.key);
            assert!(!e.lines.is_empty() && !e.route.is_empty(), "{}", e.key);
        }
    }
}
