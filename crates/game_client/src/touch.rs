//! Touch controls: floating stick, camera drag, pinch zoom, tap to target and on-screen buttons.
//! Gameplay systems read keys, so the buttons inject key presses instead of duplicating command
//! logic; the stick and drag write the same `PlayerInput` / `OrbitCamera` state the mouse does.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::camera::OrbitCamera;
use crate::config::CAM_ORBIT_SENS;
use crate::hud::{ACCENT, KEY_RIM, KEY_TOP, KeyHint, label};
use crate::input::{Paused, PlayerInput, collect_input};
use crate::keybinds::{Action, Keybinds};
use crate::targeting::{Target, pick_unit};
use crate::units::Unit;
use bevy::input::InputSystems;
use bevy::input::touch::TouchInput;
use bevy::prelude::*;
use bevy::ui::UiSystems;
use std::collections::HashMap;

// Sizes below are in HUD px (scaled by `UiScale`), not window px: with the phone resolution cap the
// window's logical px are not CSS px, but the HUD scale keeps buttons the same physical size.
/// Stick travel that equals full speed.
const STICK_RADIUS: f32 = 90.0;
const STICK_DEADZONE: f32 = 0.15;
/// A touch that moves less than this and ends quickly is a tap, not a drag.
const TAP_MAX_TRAVEL: f32 = 16.0;
const TAP_MAX_SECS: f32 = 0.3;
/// Touches that start in the left part of the screen steer; the rest look around.
const STICK_ZONE: f32 = 0.45;
/// Finger drags are coarser than a mouse, so the orbit is a bit more sensitive.
const LOOK_SENS: f32 = CAM_ORBIT_SENS * 1.4;
/// Where the idle stick hint sits, measured from the bottom-left corner.
const HINT_POS: Vec2 = Vec2::new(140.0, 190.0);

/// Whether the touch layout is on. Set at startup on coarse-pointer devices and on the first
/// touch anywhere, so a touch laptop only switches when the screen is actually used.
#[derive(Resource, Default)]
pub struct TouchMode {
    pub active: bool,
    /// Phone-class device (coarse primary pointer): also gets the cheaper render preset.
    pub mobile: bool,
}

enum Role {
    Stick,
    Look {
        t0: f32,
    },
    /// Started on a HUD button; the button owns it.
    Ui,
}

#[derive(Resource, Default)]
pub struct TouchState {
    roles: HashMap<u64, Role>,
    /// True while the stick is outside its dead zone.
    pub move_active: bool,
    origin: Vec2,
    offset: Vec2,
    pinch_prev: Option<f32>,
    /// Keys injected last frame; released at the start of the next one.
    release: Vec<KeyCode>,
}

#[derive(Component)]
enum TouchButton {
    /// Injects whatever key the action is currently bound to.
    Act(Action),
    /// Esc, which is not rebindable.
    Key(KeyCode),
    Jump,
}

#[derive(Component)]
struct TouchHud;

#[derive(Component)]
struct StickBase;

#[derive(Component)]
struct StickThumb;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TouchMode {
            active: is_mobile() || std::env::var_os("WOW_TOUCH").is_some(),
            mobile: is_mobile(),
        })
        .init_resource::<TouchState>()
        .add_systems(Startup, spawn_hud)
        .add_systems(
            PreUpdate,
            (gestures, press_buttons)
                .chain()
                .after(InputSystems)
                .after(UiSystems::Focus)
                .after(collect_input),
        )
        .add_systems(
            Update,
            (detect_touch, apply_mode, draw_stick, mobile_quality),
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn is_mobile() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(pointer: coarse)").ok().flatten())
        .is_some_and(|m| m.matches())
}

#[cfg(not(target_arch = "wasm32"))]
fn is_mobile() -> bool {
    false
}

/// Phones render at most 1.5x the CSS size: a 3x device pixel ratio would shade 4x the pixels
/// for no visible gain at arm's length.
pub fn initial_resolution() -> bevy::window::WindowResolution {
    let res = bevy::window::WindowResolution::default();
    #[cfg(target_arch = "wasm32")]
    if is_mobile() {
        let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio() as f32);
        return res.with_scale_factor_override(dpr.clamp(1.0, 1.5));
    }
    res
}

fn detect_touch(mut touches: MessageReader<TouchInput>, mut mode: ResMut<TouchMode>) {
    if !mode.active && touches.read().next().is_some() {
        mode.active = true;
    }
    touches.clear();
}

/// Cheaper preset for phones: no shadow map, no bloom pass. Desktop is untouched.
fn mobile_quality(
    mode: Res<TouchMode>,
    mut commands: Commands,
    mut lights: Query<&mut DirectionalLight, Added<DirectionalLight>>,
    blooms: Query<Entity, Added<bevy::post_process::bloom::Bloom>>,
) {
    if !mode.mobile {
        return;
    }
    for mut l in &mut lights {
        l.shadow_maps_enabled = false;
    }
    for e in &blooms {
        commands
            .entity(e)
            .remove::<bevy::post_process::bloom::Bloom>();
    }
}

fn gestures(
    time: Res<Time>,
    touches: Res<Touches>,
    mode: Res<TouchMode>,
    paused: Res<Paused>,
    ui_scale: Res<UiScale>,
    window: Single<&Window>,
    fresh_ui: Query<&Interaction, Changed<Interaction>>,
    colliders: Res<Colliders>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    units: Query<(Entity, &Transform, &Unit)>,
    mut st: ResMut<TouchState>,
    binds: Res<Keybinds>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut input: ResMut<PlayerInput>,
    mut cam: Single<&mut OrbitCamera>,
    mut target: ResMut<Target>,
) {
    for k in st.release.drain(..) {
        keys.release(k);
    }
    st.move_active = false;
    if !mode.active {
        return;
    }
    if paused.0 {
        st.roles.clear();
        st.pinch_prev = None;
        st.offset = Vec2::ZERO;
        return;
    }
    let now = time.elapsed_secs();
    let s = ui_scale.0.max(0.01);
    let radius = STICK_RADIUS * s;
    let ui_pressed = fresh_ui.iter().any(|i| *i == Interaction::Pressed);
    for t in touches.iter_just_pressed() {
        let stick_free = !st.roles.values().any(|r| matches!(r, Role::Stick));
        let role = if ui_pressed {
            Role::Ui
        } else if t.position().x < window.width() * STICK_ZONE && stick_free {
            st.origin = t.position();
            st.offset = Vec2::ZERO;
            Role::Stick
        } else {
            Role::Look { t0: now }
        };
        st.roles.insert(t.id(), role);
    }

    let mut looks: Vec<(Vec2, Vec2)> = Vec::new();
    for t in touches.iter() {
        match st.roles.get(&t.id()) {
            Some(Role::Stick) => {
                st.offset = (t.position() - st.origin).clamp_length_max(radius);
            }
            Some(Role::Look { .. }) => looks.push((t.delta(), t.position())),
            _ => {}
        }
    }
    match looks.as_slice() {
        [(delta, _)] => {
            cam.orbit(-delta.x / s * LOOK_SENS, delta.y / s * LOOK_SENS);
            st.pinch_prev = None;
        }
        [(_, a), (_, b), ..] => {
            let dist = a.distance(*b);
            if let Some(prev) = st.pinch_prev
                && dist > 1.0
            {
                cam.zoom_by(prev / dist);
            }
            st.pinch_prev = Some(dist);
        }
        [] => st.pinch_prev = None,
    }

    let stick = st.offset / radius;
    let mag = stick.length();
    if mag > STICK_DEADZONE {
        // Remap so speed ramps from zero at the dead-zone edge instead of jumping.
        let v = stick / mag * ((mag - STICK_DEADZONE) / (1.0 - STICK_DEADZONE));
        input.forward = (input.forward - v.y).clamp(-1.0, 1.0);
        input.strafe = (input.strafe + v.x).clamp(-1.0, 1.0);
        // Strafe mode like RMB steering: the body always faces where the camera looks.
        input.face_yaw = Some(cam.yaw());
        st.move_active = true;
    }

    for t in touches.iter_just_released() {
        let role = st.roles.remove(&t.id());
        if let Some(Role::Stick) = role {
            st.offset = Vec2::ZERO;
        }
        let tap = matches!(role, Some(Role::Look { t0 }) if now - t0 < TAP_MAX_SECS)
            && t.position().distance(t.start_position()) < TAP_MAX_TRAVEL * s;
        if !tap {
            continue;
        }
        let Some(picked) = pick_unit(t.position(), *camera, &colliders.0, &units) else {
            continue;
        };
        // A second tap on the current target is the touch "talk / loot / engage" (F).
        if target.0 == Some(picked) {
            let f = binds.key(Action::Interact);
            keys.press(f);
            st.release.push(f);
        }
        target.0 = Some(picked);
    }
    for t in touches.iter_just_canceled() {
        if let Some(Role::Stick) = st.roles.remove(&t.id()) {
            st.offset = Vec2::ZERO;
        }
    }
}

fn press_buttons(
    buttons: Query<(&TouchButton, &Interaction), Changed<Interaction>>,
    paused: Res<Paused>,
    binds: Res<Keybinds>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut input: ResMut<PlayerInput>,
    mut st: ResMut<TouchState>,
) {
    for (button, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            // Esc must work while paused (it resumes); movement must not.
            TouchButton::Jump if !paused.0 => input.jump = true,
            TouchButton::Jump => {}
            TouchButton::Act(a) => {
                let k = binds.key(*a);
                keys.press(k);
                st.release.push(k);
            }
            TouchButton::Key(k) => {
                keys.press(*k);
                st.release.push(*k);
            }
        }
    }
}

fn spawn_hud(mut commands: Commands) {
    let ring = |size: f32, alpha: f32| {
        (
            Node {
                position_type: PositionType::Absolute,
                width: px(size),
                height: px(size),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(alpha)),
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, alpha * 0.5)),
        )
    };
    commands.spawn((StickBase, ring(STICK_RADIUS * 2.0, 0.5)));
    commands.spawn((StickThumb, ring(STICK_RADIUS * 0.8, 0.8)));

    let button = |text: &str, tb: TouchButton, w: f32, h: f32| {
        (
            tb,
            Button,
            Node {
                width: px(w),
                height: px(h),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BorderColor::all(KEY_RIM),
            BackgroundColor(KEY_TOP),
            children![(
                label(text.to_string(), 18.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0))
            )],
        )
    };
    // Right thumb cluster, above the action bar.
    commands
        .spawn((
            TouchHud,
            Node {
                position_type: PositionType::Absolute,
                right: px(14),
                bottom: px(116),
                width: px(190),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(8),
                row_gap: px(8),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|p| {
            for (t, k) in [
                ("T", Action::AutoAttack),
                ("C", Action::Coffee),
                ("F", Action::Interact),
            ] {
                p.spawn(button(t, TouchButton::Act(k), 56.0, 56.0));
            }
            p.spawn(button("Jump", TouchButton::Jump, 120.0, 56.0));
        });
    // Window shortcuts along the top centre; the container itself ignores touches.
    commands
        .spawn((
            TouchHud,
            Node {
                position_type: PositionType::Absolute,
                top: px(6),
                left: px(0),
                right: px(0),
                justify_content: JustifyContent::Center,
                column_gap: px(8),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|p| {
            for (t, k) in [
                ("J", TouchButton::Act(Action::QuestLog)),
                ("N", TouchButton::Act(Action::Map)),
                ("B", TouchButton::Act(Action::Trophies)),
                ("M", TouchButton::Act(Action::Mute)),
                ("Esc", TouchButton::Key(KeyCode::Escape)),
            ] {
                p.spawn(button(t, k, 52.0, 40.0));
            }
        });
}

fn apply_mode(
    mode: Res<TouchMode>,
    mut hud: Query<&mut Node, (With<TouchHud>, Without<KeyHint>)>,
    mut hint: Query<&mut Node, (With<KeyHint>, Without<TouchHud>)>,
) {
    if !mode.is_changed() {
        return;
    }
    for mut n in &mut hud {
        n.display = if mode.active {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut n in &mut hint {
        n.display = if mode.active {
            Display::None
        } else {
            Display::Flex
        };
    }
}

fn draw_stick(
    mode: Res<TouchMode>,
    st: Res<TouchState>,
    window: Single<&Window>,
    ui_scale: Res<UiScale>,
    mut base: Single<&mut Node, (With<StickBase>, Without<StickThumb>)>,
    mut thumb: Single<&mut Node, (With<StickThumb>, Without<StickBase>)>,
) {
    if !mode.active {
        base.display = Display::None;
        thumb.display = Display::None;
        return;
    }
    let s = ui_scale.0.max(0.01);
    let held = st.roles.values().any(|r| matches!(r, Role::Stick));
    // Idle: a faint ring where a thumb naturally rests; held: ring under the finger.
    let origin = if held {
        st.origin
    } else {
        Vec2::new(HINT_POS.x * s, window.height() - HINT_POS.y * s)
    };
    // `size` is in HUD px; positions are window px, which the UI scale divides back out.
    let place = |n: &mut Node, centre: Vec2, size: f32| {
        n.display = Display::Flex;
        n.left = px(centre.x / s - size / 2.0);
        n.top = px(centre.y / s - size / 2.0);
        n.width = px(size);
        n.height = px(size);
    };
    place(&mut base, origin, STICK_RADIUS * 2.0);
    if held {
        place(&mut thumb, origin + st.offset, STICK_RADIUS * 0.8);
    } else {
        thumb.display = Display::None;
    }
}
