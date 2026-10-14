//! Raw devices -> one per-frame snapshot that the fixed-step simulation consumes.

use crate::config::{CLICK_DRAG_THRESHOLD, MOUSE_YAW_SENS};
use crate::keybinds::{Action, Keybinds};
use bevy::input::InputSystems;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, WindowFocused};

#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// True while a text field (the `~/coop` lobby) owns the keyboard; gameplay keys must not fire.
#[derive(Resource, Default)]
pub struct TextFocus(pub bool);

/// The `~/trophies` window (B). Lives here so Esc can close it before it opens the pause menu.
#[derive(Resource, Default)]
pub struct TrophiesOpen(pub bool);

/// The `~/career` talent window (R); Esc closes it like the others.
#[derive(Resource, Default)]
pub struct CareerOpen(pub bool);

/// Sprint planner and Sprint Review windows (`sprint.rs`); Esc closes them like the others.
#[derive(Resource, Default)]
pub struct SprintWindows {
    pub planner: bool,
    pub review: bool,
}

impl SprintWindows {
    pub fn close_one(&mut self) -> bool {
        std::mem::take(&mut self.review) || std::mem::take(&mut self.planner)
    }
}

/// Quest dialog (sim id of the giver), `~/jira` log (J) and map (N). Here for the same reason as
/// `TrophiesOpen`: Esc closes them before it clears the target or pauses.
#[derive(Resource, Default)]
pub struct QuestWindows {
    pub dialog: Option<combat_core::EntityId>,
    pub log: bool,
    pub map: bool,
}

impl QuestWindows {
    /// Closes the topmost window; false when none was open.
    pub fn close_one(&mut self) -> bool {
        if self.dialog.take().is_some() {
            true
        } else if self.log {
            self.log = false;
            true
        } else if self.map {
            self.map = false;
            true
        } else {
            false
        }
    }
}

/// Held-key state is overwritten every frame; `jump` and `mouse_yaw` are
/// latched until a fixed tick consumes them, so no input is lost when a frame
/// produces zero ticks.
#[derive(Resource, Default)]
pub struct PlayerInput {
    pub forward: f32,
    pub strafe: f32,
    pub turn: f32,
    pub jump: bool,
    pub mouse_yaw: f32,
    /// Set when RMB goes down: the character snaps to face where the camera looks.
    pub face_yaw: Option<f32>,
}

#[derive(Resource, Default)]
pub struct MouseState {
    pub left: bool,
    pub right: bool,
    pub right_just_pressed: bool,
    /// True on the frame a left press turns into a drag.
    drag_started: bool,
    /// Pixels of motion since the left button went down.
    left_travel: f32,
    /// Cursor position of a completed left click (no drag), valid for one frame.
    pub click: Option<Vec2>,
}

impl MouseState {
    pub fn any_button(&self) -> bool {
        self.left || self.right
    }

    fn left_dragging(&self) -> bool {
        self.left && self.left_travel > CLICK_DRAG_THRESHOLD
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Paused>()
            .init_resource::<TextFocus>()
            .init_resource::<TrophiesOpen>()
            .init_resource::<CareerOpen>()
            .init_resource::<QuestWindows>()
            .init_resource::<SprintWindows>()
            .init_resource::<PlayerInput>()
            .init_resource::<MouseState>()
            .add_systems(
                PreUpdate,
                (
                    release_on_focus_loss,
                    track_mouse,
                    collect_input,
                    update_cursor,
                )
                    .chain()
                    .after(InputSystems),
            );
    }
}

/// Bevy only releases keyboard state on focus loss; a button released outside
/// the window would otherwise stay "held" and keep the pointer grabbed.
fn release_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut mouse: ResMut<MouseState>,
    mut cursor: Single<&mut CursorOptions>,
) {
    if focus.read().any(|e| !e.focused) {
        // reset_all, not release_all: no phantom just_released click.
        buttons.reset_all();
        *mouse = MouseState::default();
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn track_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window>,
    mut mouse: ResMut<MouseState>,
) {
    mouse.click = None;
    mouse.right = buttons.pressed(MouseButton::Right);
    mouse.right_just_pressed = buttons.just_pressed(MouseButton::Right);
    mouse.left = buttons.pressed(MouseButton::Left);
    if buttons.just_pressed(MouseButton::Left) {
        mouse.left_travel = 0.0;
    }
    let was_dragging = mouse.left_dragging();
    let released = buttons.just_released(MouseButton::Left);
    // The release frame still carries motion that belongs to the gesture.
    if mouse.left || released {
        mouse.left_travel += motion.delta.length();
    }
    mouse.drag_started = !was_dragging && mouse.left_dragging();
    // With RMB held the cursor is locked, so its position is stale.
    if released && mouse.left_travel <= CLICK_DRAG_THRESHOLD && !mouse.right {
        mouse.click = window.cursor_position();
    }
}

pub(crate) fn collect_input(
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    text_focus: Res<TextFocus>,
    binds: Res<Keybinds>,
    mut input: ResMut<PlayerInput>,
) {
    if paused.0 || text_focus.0 {
        *input = PlayerInput::default();
        return;
    }
    let axis = |pos: Action, neg: Action| {
        f32::from(u8::from(binds.held(&keys, pos))) - f32::from(u8::from(binds.held(&keys, neg)))
    };
    let mut forward = axis(Action::Forward, Action::Backward);
    if mouse.left && mouse.right {
        forward = 1.0;
    }
    let ad = axis(Action::TurnRight, Action::TurnLeft);
    let qe = axis(Action::StrafeRight, Action::StrafeLeft);
    // WoW: while RMB steers the character, A/D strafe instead of turning.
    let (turn, strafe) = if mouse.right {
        (0.0, qe + ad)
    } else {
        (-ad, qe)
    };
    input.forward = forward;
    input.strafe = strafe.clamp(-1.0, 1.0);
    input.turn = turn;
    input.jump |= binds.just(&keys, Action::Jump);
    if mouse.right {
        input.mouse_yaw -= motion.delta.x * MOUSE_YAW_SENS;
    }
}

/// Hide and capture the pointer only while a drag is in progress, so a plain
/// left click keeps a usable cursor position for target picking.
fn update_cursor(
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    mut cursor: Single<&mut CursorOptions>,
) {
    let want = !paused.0 && (mouse.right || mouse.left_dragging());
    if want {
        // Re-request at the start of each gesture: browsers drop pointer lock
        // on Esc without telling Bevy, so a mode-equality guard would never retry.
        if mouse.right_just_pressed || mouse.drag_started {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    } else if cursor.grab_mode != CursorGrabMode::None || !cursor.visible {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}
