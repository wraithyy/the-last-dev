//! Env-gated QA hooks, inert unless the variables are set:
//! - `WOW_SCREENSHOT=<path.png>`: save the window after `WOW_SHOT_DELAY` seconds (default 4) and exit.
//!   `WOW_SHOT_TIMES=t1,t2,..` instead saves one `<path>_<n>.png` per time (seconds since start).
//! - `WOW_LINEUP=1|2`: bare stage with half of the cast in two rows (see `units.rs`); `0` leaves the stage empty.
//! - `WOW_LINEUP=3`: only the campus/data center/lab units, three rows (use `WOW_CAM_DIST=20`).
//! - `WOW_LINEUP=4`: the four class skins with their kits, plus the Backend's five hair stages.
//! - `WOW_CLASS=fullstack|frontend|backend|devops`: play that class and skip the role picker;
//!   `WOW_PICKER=1` opens the picker even when a role is stored (see `classes.rs`).
//!   `WOW_SHOT_SIZE=<w>x<h>` changes the target size (e.g. 2560x1080 for ultrawide checks).
//! - `WOW_ZONE=<id>`: start in a zone (campus, tower/1..5, datacenter/1..2, lab/1..2), sim-backed and
//!   playable, see `game.rs` (also `WOW_QUESTS`, `WOW_UNLOCK_ALL`); works with `WOW_TOPDOWN`
//!   (140 frames the whole campus) and `WOW_PLAYER_AT=x,z,yaw`.
//! - `WOW_JIRA=1`, `WOW_MAP=1`: open the `~/jira` log or the map at start.
//! - `WOW_KILL_BOSS=<secs>`: the zone boss dies (as a player kill) that many seconds in.
//! - `WOW_BOSS_PCT=<pct>@<secs>`: the zone boss drops to that HP percent at that time (phase shots).
//! - `WOW_FPS=1`: log average frame time and drawn mesh count every 3 s (vsync caps the rate, so read the ms).
//! - `WOW_HOVER_SLOT=<0-9>`: show that action slot's tooltip without a pointer.
//!
//! Floor, level and start-position hooks live in `game.rs`, the autofight bot in `combat.rs`.
//! - `WOW_TALK=1`: talk to the nearest friendly NPC (within reach) two seconds in.
//! - `WOW_CAM_DIST=<m>`: start with the camera zoomed to that distance; `WOW_CAM_PITCH=<deg>` tilts it.
//! - `WOW_GIVE_ITEMS=1`: grant a sample set of trophies at start (stacks included).
//! - `WOW_TROPHIES=1`: open the `~/trophies` window at start.
//! - `WOW_LOOT_DEMO=1`: show the loot window and log lines for three sample items (display only; see `hud/feed.rs`).
//! - `WOW_COFFEE=<secs>`: drain mana to 30 and start a Coffee Break that many seconds in (1 if not a number).
//! - `WOW_AUTORUN=1`: hold "forward" so animation and camera can be checked without input.
//! - `WOW_METER_MODE=0|1|2`, `WOW_METER_SCOPE=overall`: initial `~/metrics` mode and scope (open it with `WOW_KEYS=t:KeyK`).
//! - `WOW_TOUCH=1`: start in the touch layout (native only; the web build detects a coarse pointer);
//!   `WOW_TOUCH_PANEL=chat|log` also expands that log panel.
//! - `WOW_KEYS=t:Key,t:Key:hold,t:yaw=<rad>`: scripted input for two-client co-op shots. At `t`
//!   seconds a key (`KeyW`, `Digit6`, `F2`, `Tab`, ...) is tapped, or held for `hold` seconds,
//!   or the player turns to `yaw`. Goes through the real input paths (online too).

use crate::camera::OrbitCamera;
use crate::combat::{CombatSim, PendingCommands, SimReady};
use crate::input::{PlayerInput, TrophiesOpen};
use crate::player::PlayerBody;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use combat_core::{Command, distance};
use std::env;

pub struct DebugPlugin;

/// `WOW_SHOT_SIZE=<w>x<h>` (default 1280x720): size of the offscreen screenshot target.
pub fn shot_size() -> (u32, u32) {
    env::var("WOW_SHOT_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .filter(|&(w, h): &(u32, u32)| (320..=7680).contains(&w) && (240..=4320).contains(&h))
        .unwrap_or((1280, 720))
}

/// True while screenshots go to the offscreen target (UI and plates use its size, not the window's).
pub fn shooting() -> bool {
    env::var_os("WOW_SCREENSHOT").is_some()
}

#[derive(Resource)]
struct ShotPlan {
    path: String,
    /// Offscreen target: an occluded macOS window never presents, so a window capture is black.
    target: Handle<Image>,
    at: f32,
    taken: bool,
    /// Extra shots (see `WOW_SHOT_TIMES`); the plan ends after the last one.
    times: Vec<f32>,
    next: usize,
}

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(path) = env::var("WOW_SCREENSHOT") {
            let at = env::var("WOW_SHOT_DELAY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0);
            let target =
                app.world_mut()
                    .resource_mut::<Assets<Image>>()
                    .add(Image::new_target_texture(
                        shot_size().0,
                        shot_size().1,
                        TextureFormat::Rgba8UnormSrgb,
                        None,
                    ));
            let times: Vec<f32> = env::var("WOW_SHOT_TIMES")
                .map(|v| v.split(',').filter_map(|p| p.trim().parse().ok()).collect())
                .unwrap_or_default();
            app.insert_resource(ShotPlan {
                path,
                target,
                at,
                taken: false,
                times,
                next: 0,
            })
            .add_systems(Update, (retarget_camera, screenshot_and_exit));
        }
        if env::var("WOW_LINEUP").is_ok() {
            app.add_systems(
                PostStartup,
                |mut player: Single<(&mut PlayerBody, &mut Visibility)>,
                 mut cam: Single<&mut OrbitCamera>| {
                    player.0.teleport_facing(Vec3::new(0.0, 0.0, 2.0), 0.0);
                    cam.set_yaw(0.0);
                    // The class lineup is about the skins; the player would stand in front of them.
                    if env::var("WOW_LINEUP").is_ok_and(|v| v == "4") {
                        *player.1 = Visibility::Hidden;
                    }
                },
            );
        }
        if env::var("WOW_FPS").is_ok() {
            app.add_systems(Update, log_frame_stats);
        }
        if env::var("WOW_TALK").is_ok() {
            app.add_systems(Update, talk_once.in_set(SimReady));
        }
        if let Some(dist) = env::var("WOW_CAM_DIST").ok().and_then(|v| v.parse().ok()) {
            app.add_systems(PostStartup, move |mut cam: Single<&mut OrbitCamera>| {
                cam.set_zoom(dist);
            });
        }
        if let Some(deg) = env::var("WOW_CAM_PITCH")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.add_systems(PostStartup, move |mut cam: Single<&mut OrbitCamera>| {
                cam.set_pitch(deg.to_radians());
            });
        }
        if let Some(deg) = env::var("WOW_CAM_YAW_OFFSET")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            // After the floor load has set the start heading.
            app.add_systems(
                Update,
                move |time: Res<Time>, mut cam: Single<&mut OrbitCamera>, mut done: Local<bool>| {
                    if !*done && time.elapsed_secs() > 3.0 {
                        *done = true;
                        cam.add_yaw(deg.to_radians());
                    }
                },
            );
        }
        if let Some(slot) = env::var("WOW_HOVER_SLOT").ok().and_then(|v| v.parse().ok()) {
            app.insert_resource(crate::hud::DebugHoverSlot(slot));
        }
        if env::var("WOW_GIVE_ITEMS").is_ok() {
            app.add_systems(Update, give_items_once.in_set(SimReady));
        }
        if let Ok(v) = env::var("WOW_COFFEE") {
            let at = v.parse().unwrap_or(1.0);
            app.add_systems(
                Update,
                (move |time: Res<Time>,
                       sim: ResMut<CombatSim>,
                       pending: ResMut<PendingCommands>,
                       done: Local<bool>| {
                    coffee_once(at, time, sim, pending, done)
                })
                .in_set(SimReady),
            );
        }
        if let Some(at) = env::var("WOW_KILL_BOSS")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            app.add_systems(
                Update,
                (move |time: Res<Time>, mut sim: ResMut<CombatSim>, mut done: Local<bool>| {
                    if *done || time.elapsed_secs() < at {
                        return;
                    }
                    *done = true;
                    let boss = sim.0.units().find(|u| u.boss && u.alive).map(|u| u.id);
                    if let Some(id) = boss {
                        sim.0.debug_kill(id);
                    }
                })
                .in_set(SimReady),
            );
        }
        if let Some((pct, at)) = env::var("WOW_BOSS_PCT").ok().and_then(|v| {
            let (p, t) = v.split_once('@')?;
            Some((p.parse::<u32>().ok()?, t.parse::<f32>().ok()?))
        }) {
            app.add_systems(
                Update,
                (move |time: Res<Time>, mut sim: ResMut<CombatSim>, mut done: Local<bool>| {
                    if *done || time.elapsed_secs() < at {
                        return;
                    }
                    *done = true;
                    let boss = sim
                        .0
                        .units()
                        .find(|u| u.boss && u.alive)
                        .map(|u| (u.id, u.max_hp));
                    if let Some((id, max)) = boss {
                        sim.0.debug_set_hp(id, max * pct / 100);
                    }
                })
                .in_set(SimReady),
            );
        }
        if env::var("WOW_TROPHIES").is_ok() {
            app.insert_resource(TrophiesOpen(true));
        }
        if env::var("WOW_JIRA").is_ok() || env::var("WOW_MAP").is_ok() {
            app.insert_resource(crate::input::QuestWindows {
                dialog: None,
                log: env::var("WOW_JIRA").is_ok(),
                map: env::var("WOW_MAP").is_ok(),
            });
        }
        if let Ok(v) = env::var("WOW_KEYS") {
            let script = KeyScript {
                steps: parse_key_script(&v),
                held: Vec::new(),
                next: 0,
            };
            app.insert_resource(script)
                .add_systems(
                    PreUpdate,
                    run_key_script
                        .after(bevy::input::InputSystems)
                        .before(bevy::ui::UiSystems::Focus),
                )
                .add_systems(FixedPreUpdate, face_from_script);
        }
        if env::var("WOW_AUTORUN").is_ok() {
            // Fixed pre-update runs after the per-frame input snapshot and before the sim consumes it.
            app.add_systems(FixedPreUpdate, |mut input: ResMut<PlayerInput>| {
                input.forward = 1.0;
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScriptAction {
    Key(KeyCode, f32),
    Face(f32),
}

#[derive(Resource)]
struct KeyScript {
    steps: Vec<(f32, ScriptAction)>,
    /// Keys held down and when to let go.
    held: Vec<(KeyCode, f32)>,
    next: usize,
}

/// One-shot facing set by the script, consumed by the movement step.
#[derive(Resource)]
struct ScriptFace(f32);

fn key_code(name: &str) -> Option<KeyCode> {
    let k = match name {
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Escape,
        "Space" => KeyCode::Space,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        _ => {
            if let Some(d) = name.strip_prefix("Digit") {
                let slot = u8::try_from((d.parse::<usize>().ok()? + 9) % 10).ok()?;
                crate::keybinds::Action::Slot(slot).default_key()
            } else {
                let c = name.strip_prefix("Key")?.chars().next()?;
                match c {
                    'A' => KeyCode::KeyA,
                    'B' => KeyCode::KeyB,
                    'D' => KeyCode::KeyD,
                    'X' => KeyCode::KeyX,
                    'Y' => KeyCode::KeyY,
                    'C' => KeyCode::KeyC,
                    'F' => KeyCode::KeyF,
                    'G' => KeyCode::KeyG,
                    'H' => KeyCode::KeyH,
                    'I' => KeyCode::KeyI,
                    'K' => KeyCode::KeyK,
                    'L' => KeyCode::KeyL,
                    'S' => KeyCode::KeyS,
                    'T' => KeyCode::KeyT,
                    'V' => KeyCode::KeyV,
                    'W' => KeyCode::KeyW,
                    _ => return None,
                }
            }
        }
    };
    Some(k)
}

/// `t:Key`, `t:Key:hold` or `t:yaw=<rad>`, comma separated; bad entries are skipped. Sorted by time.
pub fn parse_key_script(src: &str) -> Vec<(f32, ScriptAction)> {
    let mut out: Vec<(f32, ScriptAction)> = src
        .split(',')
        .filter_map(|entry| {
            let mut parts = entry.trim().split(':');
            let at: f32 = parts.next()?.trim().parse().ok()?;
            let what = parts.next()?.trim();
            let action = match what.strip_prefix("yaw=") {
                Some(y) => ScriptAction::Face(y.parse().ok()?),
                None => {
                    let hold = parts.next().map_or(Some(0.0), |h| h.trim().parse().ok())?;
                    ScriptAction::Key(key_code(what)?, hold)
                }
            };
            Some((at, action))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn run_key_script(
    mut commands: Commands,
    time: Res<Time>,
    mut script: ResMut<KeyScript>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
) {
    let now = time.elapsed_secs();
    let script = &mut *script;
    script.held.retain(|(k, until)| {
        let keep = *until > now;
        if !keep {
            keys.release(*k);
        }
        keep
    });
    while let Some(&(at, action)) = script.steps.get(script.next) {
        if at > now {
            break;
        }
        script.next += 1;
        info!("WOW_KEYS {at}: {action:?}");
        match action {
            ScriptAction::Key(k, hold) => {
                keys.press(k);
                // A tap is released on the next frame, after every system saw it.
                script.held.push((k, now + hold.max(0.001)));
            }
            ScriptAction::Face(yaw) => commands.insert_resource(ScriptFace(yaw)),
        }
    }
}

fn face_from_script(
    mut commands: Commands,
    face: Option<Res<ScriptFace>>,
    mut input: ResMut<PlayerInput>,
) {
    if let Some(f) = face {
        input.face_yaw = Some(f.0);
        commands.remove_resource::<ScriptFace>();
    }
}

/// Cap (3) is hit by the invites and exceeded by the deck, so every badge style shows up.
const SAMPLE_TROPHIES: [&str; 13] = [
    "Unread AI Strategy Deck",
    "Unread AI Strategy Deck",
    "Prompt Fragment",
    "Expired GPU Credits",
    "Mandatory Prompt Training Certificate",
    "Recurring Meeting Invite",
    "Recurring Meeting Invite",
    "Recurring Meeting Invite",
    "Sticky Note with Admin Password",
    "Org Chart (Already Outdated)",
    "Pizza Party Voucher (Expired)",
    "Logo, Slightly Rounder",
    "Lanyard of Authority",
];

fn give_items_once(mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    if !*done {
        *done = true;
        for name in SAMPLE_TROPHIES {
            sim.0.grant_trophy(name);
        }
    }
}

fn coffee_once(
    at: f32,
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    sim.0.set_player_mana(30);
    if let Some(me) = sim.0.player() {
        crate::combat::cast_coffee_break(&sim.0, &mut pending, me);
    }
}

fn talk_once(
    time: Res<Time>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    mut talked: MessageWriter<crate::combat::Talked>,
    mut done: Local<bool>,
) {
    if *done || time.elapsed_secs() < 2.0 {
        return;
    }
    *done = true;
    let Some(me) = sim.0.player().and_then(|id| sim.0.unit(id)) else {
        return;
    };
    let reach = sim.0.content().config.loot_range_m;
    let dist = |u: &combat_core::UnitState| distance(u.pos, me.pos);
    if let Some(npc) = sim
        .0
        .units()
        .filter(|u| !u.hostile && u.alive && u.id != me.id && dist(u) <= reach)
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
    {
        pending.0.push(Command::Talk { unit: npc.id });
        talked.write(crate::combat::Talked(npc.id));
    }
}

/// Average frame time and how many meshes the camera actually draws, for culling checks.
fn log_frame_stats(
    time: Res<Time>,
    meshes: Query<
        (
            &ViewVisibility,
            Option<&bevy::camera::visibility::VisibilityRange>,
        ),
        With<Mesh3d>,
    >,
    mut acc: Local<(f32, u32)>,
) {
    acc.0 += time.delta_secs();
    acc.1 += 1;
    if acc.0 >= 3.0 {
        let drawn = meshes.iter().filter(|(v, _)| v.get()).count();
        let ranged = meshes.iter().filter(|(_, r)| r.is_some()).count();
        info!(
            "frame stats: {:.2} ms/frame ({:.0} fps), {drawn} of {} meshes drawn ({ranged} with a visibility range)",
            acc.0 / acc.1 as f32 * 1000.0,
            acc.1 as f32 / acc.0,
            meshes.iter().count(),
        );
        *acc = (0.0, 0);
    }
}

fn screenshot_and_exit(
    mut commands: Commands,
    time: Res<Time>,
    mut plan: ResMut<ShotPlan>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    if !plan.times.is_empty() {
        if let Some(&at) = plan.times.get(plan.next)
            && t >= at
        {
            let path = match plan.path.rsplit_once('.') {
                Some((stem, ext)) => format!("{stem}_{}.{ext}", plan.next),
                None => format!("{}_{}", plan.path, plan.next),
            };
            commands
                .spawn(Screenshot::image(plan.target.clone()))
                .observe(save_to_disk(path));
            plan.next += 1;
        } else if plan.next >= plan.times.len() && t >= plan.times[plan.times.len() - 1] + 2.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if !plan.taken && t >= plan.at {
        plan.taken = true;
        commands
            .spawn(Screenshot::image(plan.target.clone()))
            .observe(save_to_disk(plan.path.clone()));
    } else if plan.taken && t >= plan.at + 2.0 {
        exit.write(AppExit::Success);
    }
}

fn retarget_camera(
    mut commands: Commands,
    plan: Res<ShotPlan>,
    // Only the play camera: preview cameras (role picker, character sheet) render elsewhere.
    cams: Query<Entity, (Added<Camera3d>, With<crate::camera::OrbitCamera>)>,
) {
    for cam in &cams {
        commands
            .entity(cam)
            // Without this marker the UI would find no camera aimed at the window and vanish from the shot.
            .insert((
                RenderTarget::Image(plan.target.clone().into()),
                IsDefaultUiCamera,
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_scripts_parse_taps_holds_and_turns_in_time_order() {
        let s =
            parse_key_script("5:Digit6, 1:KeyW:2.5, 3:yaw=-1.5, 4:F2, 9:Nope, x:KeyG, 6:Digit0");
        assert_eq!(
            s,
            vec![
                (1.0, ScriptAction::Key(KeyCode::KeyW, 2.5)),
                (3.0, ScriptAction::Face(-1.5)),
                (4.0, ScriptAction::Key(KeyCode::F2, 0.0)),
                (5.0, ScriptAction::Key(KeyCode::Digit6, 0.0)),
                (6.0, ScriptAction::Key(KeyCode::Digit0, 0.0)),
            ]
        );
    }
}
