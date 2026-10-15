//! Touch controls: floating stick, camera drag, pinch zoom, tap to target and on-screen buttons.
//! Gameplay systems read keys, so the buttons inject key presses instead of duplicating command
//! logic; the stick and drag write the same `PlayerInput` / `OrbitCamera` state the mouse does.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::arena::Colliders;
use crate::camera::OrbitCamera;
use crate::config::CAM_ORBIT_SENS;
use crate::hud::{ACCENT, KEY_RIM, KEY_TOP, KeyHint, PANEL_EDGE, label, panel_bg};
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

/// Which touch overlays are open: the menu drawer and the "more" tray (sprint, ride, gadgets).
#[derive(Resource, Default)]
pub struct TouchUi {
    pub menu: bool,
    pub more: bool,
}

// Touch HUD geometry in HUD px. Everything on the right is measured from the bottom-right corner
// and sized for the smallest supported canvas (~1000 x 500 HUD px, i.e. an 844x390 phone): the
// stick owns x < 230 on the left, the action bar x 272..728, the thumb cluster the rest.
/// Side of an arc or tray button. 64 HUD px stays above 44 physical px down to a 0.69 HUD scale.
pub const BTN: f32 = 64.0;
/// The primary (auto attack) key is the accent key scaled up around its centre.
pub const PRIMARY_SCALE: f32 = 1.4;
const PRIMARY: Vec2 = Vec2::new(72.0, 78.0);
/// Far enough that four buttons 30 degrees apart do not touch.
const ARC_RADIUS: f32 = 139.0;
/// Where the "more" tray starts, above the action bar and left of the arc.
const TRAY_RIGHT: f32 = 256.0;
const TRAY_BOTTOM: f32 = 156.0;
const TRAY_GAP: f32 = 8.0;
/// Action bar slots on touch; 58 HUD px is 45 physical px at the smallest scale.
pub const SLOT_W: f32 = 58.0;
pub const SLOT_H: f32 = 62.0;
/// Top row left of the minimap: menu, then the log toggle.
const TOP_ROW: f32 = 8.0;
const TOP_BTN_H: f32 = 58.0;
const MENU_W: f32 = 62.0;
const MENU_RIGHT: f32 = 12.0 + crate::hud::MINIMAP_WIDTH + 8.0;
pub const LOG_RIGHT: f32 = MENU_RIGHT + MENU_W + 8.0;
pub const LOG_TOP: f32 = TOP_ROW;
pub const LOG_SIZE: f32 = TOP_BTN_H;

/// Centre of the primary button, from the bottom-right corner.
pub fn primary() -> Vec2 {
    PRIMARY
}

/// Centre of an arc slot around the primary: 180 degrees is left of it, 90 straight above.
pub fn arc(deg: f32) -> Vec2 {
    let a = deg.to_radians();
    Vec2::new(
        PRIMARY.x - ARC_RADIUS * a.cos(),
        PRIMARY.y + ARC_RADIUS * a.sin(),
    )
}

/// Centre of tray slot `i` (0 is nearest the arc), from the bottom-right corner.
pub fn tray(i: usize) -> Vec2 {
    Vec2::new(
        TRAY_RIGHT + BTN / 2.0 + i as f32 * (BTN + TRAY_GAP),
        TRAY_BOTTOM + BTN / 2.0,
    )
}

/// `right` of the gadget bar when it sits after the two accent keys in the tray.
pub const GADGET_TRAY_RIGHT: f32 = TRAY_RIGHT + 2.0 * (BTN + TRAY_GAP);
pub const GADGET_TRAY_BOTTOM: f32 = TRAY_BOTTOM;

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
    /// Opens or closes the menu drawer.
    Menu,
    /// Opens or closes the tray with sprint, ride and gadgets.
    More,
    /// The scrim behind the drawer: a tap outside closes it.
    Dismiss,
}

/// A drawer entry: pressing it also closes the drawer.
#[derive(Component)]
struct MenuItem;

/// The drawer's scrim, shown only while `TouchUi::menu` is set.
#[derive(Component)]
struct MenuRoot;

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
        // QA hook: `WOW_TOUCH_UI=menu|more` starts with the drawer or the tray open.
        .insert_resource(match std::env::var("WOW_TOUCH_UI").as_deref() {
            Ok("menu") => TouchUi {
                menu: true,
                more: false,
            },
            Ok("more") => TouchUi {
                menu: false,
                more: true,
            },
            _ => TouchUi::default(),
        })
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
            (detect_touch, apply_mode, show_menu, draw_stick, tag_spots),
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

/// Onboarding highlight anchors on the touch buttons it points at.
fn tag_spots(mut commands: Commands, buttons: Query<(Entity, &TouchButton), Added<TouchButton>>) {
    for (e, b) in &buttons {
        // The drawer and the tray hide the buttons a tutorial step points at, so the step
        // highlights the button that opens them.
        let spot = match b {
            TouchButton::Act(a) => *a,
            TouchButton::Menu => Action::Map,
            TouchButton::More => Action::Scooter,
            _ => continue,
        };
        commands
            .entity(e)
            .insert(crate::onboarding::Spot::Touch(spot));
    }
}

fn detect_touch(mut touches: MessageReader<TouchInput>, mut mode: ResMut<TouchMode>) {
    if !mode.active && touches.read().next().is_some() {
        mode.active = true;
    }
    touches.clear();
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
    buttons: Query<(&TouchButton, &Interaction, Has<MenuItem>), Changed<Interaction>>,
    paused: Res<Paused>,
    binds: Res<Keybinds>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut input: ResMut<PlayerInput>,
    mut st: ResMut<TouchState>,
    mut ui: ResMut<TouchUi>,
) {
    for (button, interaction, in_menu) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if in_menu {
            ui.menu = false;
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
            // One overlay at a time: the tray and the drawer would stack on the same thumb.
            TouchButton::Menu => {
                ui.menu = !ui.menu;
                ui.more = false;
            }
            TouchButton::More => {
                ui.more = !ui.more;
                ui.menu = false;
            }
            TouchButton::Dismiss => ui.menu = false,
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
    // Thumb cluster: auto attack, coffee and sprint/ride/gadgets are accent keys that
    // `hud::actionbar` places around `primary()`; the rest of the arc is built here.
    for (text, tb, c) in [
        ("Jump", TouchButton::Jump, arc(150.0)),
        ("F", TouchButton::Act(Action::Interact), arc(120.0)),
        ("More", TouchButton::More, arc(90.0)),
    ] {
        let (tb, btn, mut node, border, bg, kids) = button(text, tb, BTN, BTN);
        node.position_type = PositionType::Absolute;
        node.right = px(c.x - BTN / 2.0);
        node.bottom = px(c.y - BTN / 2.0);
        node.display = Display::None;
        commands.spawn((TouchHud, tb, btn, node, border, bg, kids));
    }
    // Menu and log toggle sit left of the minimap; the toggle lives in `hud::logs`.
    let (tb, btn, mut node, border, bg, kids) =
        button("Menu", TouchButton::Menu, MENU_W, TOP_BTN_H);
    node.position_type = PositionType::Absolute;
    node.right = px(MENU_RIGHT);
    node.top = px(TOP_ROW);
    node.display = Display::None;
    // Above the pause overlay, so the drawer's Pause entry can still resume.
    commands.spawn((TouchHud, tb, btn, node, border, bg, kids, GlobalZIndex(55)));

    // Drawer: a dimmed scrim (tap outside closes) around a card with the rarely used windows.
    commands
        .spawn((
            MenuRoot,
            TouchButton::Dismiss,
            Button,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(56),
        ))
        .with_children(|scrim| {
            scrim
                .spawn((
                    // A button without behaviour, so taps on the card's padding do not dismiss.
                    Button,
                    Node {
                        width: px(4.0 * 112.0 + 3.0 * 8.0 + 36.0),
                        padding: UiRect::all(px(14)),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(8),
                        row_gap: px(8),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(12)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    panel_bg(),
                ))
                .with_children(|card| {
                    for (t, k) in [
                        ("Quests", TouchButton::Act(Action::QuestLog)),
                        ("Map", TouchButton::Act(Action::Map)),
                        ("Trophies", TouchButton::Act(Action::Trophies)),
                        ("Career", TouchButton::Act(Action::Career)),
                        ("Gear", TouchButton::Act(Action::CharacterSheet)),
                        ("Pets", TouchButton::Act(Action::Pets)),
                        ("Sound", TouchButton::Act(Action::Mute)),
                        ("Pause", TouchButton::Key(KeyCode::Escape)),
                    ] {
                        card.spawn((MenuItem, button(t, k, 112.0, 58.0)));
                    }
                });
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

fn show_menu(mode: Res<TouchMode>, ui: Res<TouchUi>, mut root: Single<&mut Node, With<MenuRoot>>) {
    let want = if mode.active && ui.menu {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != want {
        root.display = want;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn gap(a: Vec2, b: Vec2, ra: f32, rb: f32) -> f32 {
        a.distance(b) - ra - rb
    }

    #[test]
    fn thumb_cluster_buttons_do_not_touch() {
        let half = BTN / 2.0;
        let arc: Vec<Vec2> = [180.0, 150.0, 120.0, 90.0].map(arc).to_vec();
        for (i, a) in arc.iter().enumerate() {
            let primary = PRIMARY_SCALE * BTN / 2.0;
            assert!(
                gap(*a, PRIMARY, half, primary) > 0.0,
                "arc {i} hits primary"
            );
            for b in &arc[i + 1..] {
                assert!(gap(*a, *b, half, half) > 0.0, "arc buttons overlap");
            }
            assert!(
                a.x - half >= 0.0 && a.y - half >= 0.0,
                "arc leaves the screen"
            );
        }
    }

    #[test]
    fn tray_clears_the_arc_and_the_action_bar() {
        let half = BTN / 2.0;
        for i in 0..2 {
            for deg in [180.0, 150.0, 120.0, 90.0] {
                assert!(gap(tray(i), arc(deg), half, half) > 0.0);
            }
        }
    }

    // Bar top is 20 + 62 + 16 + 4 above the bottom edge; checked at compile time.
    const _: () = assert!(TRAY_BOTTOM > 102.0);
}
