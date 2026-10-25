//! Shared scrolling for every window whose content can outgrow the screen: wheel while the pointer is
//! over the list, finger drag on touch, clamping, an optional "More below" hint and a reset when the
//! window reopens or its content key changes.
//!
//! A window opts in by giving the node that clips its content `Overflow::scroll_y()` (plus a
//! `max_height` that fits the screen) and a [`ScrollList`]. The rest is automatic.
//!
//! QA hook: `WOW_SCROLL=<px>` scrolls every list by that many UI px once, the first frame it has
//! overflow (`end` jumps to the bottom), so screenshots can show a scrolled window.
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, UiGlobalTransform};
use bevy::window::PrimaryWindow;

/// Pixels per wheel notch.
const LINE_PX: f32 = 30.0;
/// Slack before the hint disappears, so it does not flicker at the very end.
/// Smallest `fit` height (HUD px) so a window never collapses on a tiny screen.
const MIN_FIT: f32 = 120.0;
const HINT_SLACK: f32 = 4.0;
const HINT_TEXT: &str = "More below: scroll or swipe the list.";

// Interaction makes the list count as UI for targeting and the touch camera, so a swipe on it never
// turns the view; the focus policy is inserted by `claim_pointer` (the Node default would win here).
#[derive(Component, Default)]
#[require(ScrollPosition, Interaction)]
pub struct ScrollList {
    /// Change it when the list shows a different page (selected quest, tab); the next frame scrolls to the top.
    pub key: u64,
    seen_key: u64,
    open: bool,
    touch: Option<u64>,
    /// Bottom margin (HUD px): the list's `max_height` is kept at "from my top edge to the screen
    /// bottom minus this", at any UiScale. For nodes with a fixed top only.
    pub fit: Option<f32>,
    /// QA hook already fired for this list.
    qa_done: bool,
    /// The "more below" pill while the list overflows (a root node, so it stays put while the
    /// list scrolls under it).
    pill: Option<Entity>,
}

impl ScrollList {
    /// A list that always fits between its top edge and the bottom of the screen.
    pub fn fit(margin: f32) -> Self {
        Self {
            fit: Some(margin),
            ..default()
        }
    }
}

/// Hashes anything into a [`ScrollList::key`].
pub fn key_of(v: &impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Put on a control that needs the finger for itself (a slider): a touch that starts on it never scrolls.
#[derive(Component)]
pub struct NoScrollDrag;

/// A text node that shows "More below" while the list has content past its bottom edge.
#[derive(Component)]
pub struct ScrollHint(pub Entity);

/// The pill every overflowing list shows along its bottom edge unless it has a `ScrollHint`.
#[derive(Component)]
struct MorePill;

/// True while the pointer is over any open scroll list; the camera skips the wheel zoom then.
#[derive(Resource, Default)]
pub struct ScrollHover(pub bool);

pub struct ScrollPlugin;

impl Plugin for ScrollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScrollHover>()
            .add_systems(Update, (claim_pointer, scroll_lists).chain());
    }
}

/// Applies a scroll step and keeps the offset inside `0..=max`.
fn step(pos: f32, delta: f32, max: f32) -> f32 {
    (pos + delta).clamp(0.0, max.max(0.0))
}

/// A list returns to the top when its window reopens or the caller changed the content key.
fn needs_reset(was_open: bool, open: bool, seen_key: u64, key: u64) -> bool {
    open && (!was_open || seen_key != key)
}

fn claim_pointer(mut commands: Commands, added: Query<Entity, Added<ScrollList>>) {
    for e in &added {
        commands.entity(e).insert(FocusPolicy::Block);
    }
}

fn scroll_lists(
    mut commands: Commands,
    mut pills: Query<(&mut Node, &mut Visibility), (With<MorePill>, Without<ScrollList>)>,
    wheel: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    ui_scale: Res<UiScale>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut hover: ResMut<ScrollHover>,
    mut lists: Query<
        (
            Entity,
            &mut ScrollList,
            &mut ScrollPosition,
            &mut Node,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        Without<MorePill>,
    >,
    mut hints: Query<(&ScrollHint, &mut Text)>,
    nodrag: Query<&Interaction, With<NoScrollDrag>>,
    cams: Query<&Camera, With<crate::camera::OrbitCamera>>,
) {
    let Some(window) = window else {
        return;
    };
    let cursor = window.physical_cursor_position();
    let sf = window.scale_factor();
    let s = ui_scale.0.max(0.01);
    // The play camera's target is the HUD's screen; in screenshots it is not the window.
    let screen_h = cams
        .iter()
        .next()
        .and_then(Camera::physical_viewport_size)
        .map_or(window.physical_height(), |v| v.y) as f32;
    let lines = match wheel.unit {
        MouseScrollUnit::Line => wheel.delta.y * LINE_PX,
        MouseScrollUnit::Pixel => wheel.delta.y,
    };
    // Nested lists: the innermost (smallest) one under the pointer or finger takes the gesture.
    let innermost = |p: Vec2| {
        lists
            .iter()
            .filter(|(.., n, tf)| n.size().y > 0.0 && n.contains_point(**tf, p))
            .min_by(|a, b| {
                let area = |n: &ComputedNode| n.size().x * n.size().y;
                area(a.4).total_cmp(&area(b.4))
            })
            .map(|l| l.0)
    };
    let wheel_target = cursor.and_then(innermost);
    let on_control = nodrag.iter().any(|i| *i == Interaction::Pressed);
    let started: Vec<(u64, Entity)> = touches
        .iter_just_pressed()
        .filter(|_| !on_control)
        .filter_map(|t| innermost(t.position() * sf).map(|e| (t.id(), e)))
        .collect();
    let mut any_hover = false;
    for (entity, mut list, mut pos, mut style, node, tf) in &mut lists {
        let open = node.size().y > 0.0;
        if !open {
            list.open = false;
            list.touch = None;
            if let Some(p) = list.pill.take() {
                commands.entity(p).despawn();
            }
            continue;
        }
        if let Some(margin) = list.fit {
            let top = tf.translation.y - node.size().y / 2.0;
            let room = screen_h - top - margin * s * sf;
            let want = px((room * node.inverse_scale_factor()).max(MIN_FIT));
            if style.max_height != want {
                style.max_height = want;
            }
        }
        let was_open = list.open;
        list.open = true;
        any_hover |= cursor.is_some_and(|p| node.contains_point(*tf, p));
        let mut dy = if wheel_target == Some(entity) {
            -lines
        } else {
            0.0
        };
        if let Some((id, _)) = started.iter().find(|(_, e)| *e == entity) {
            list.touch = Some(*id);
        }
        if let Some(id) = list.touch {
            match touches.get_pressed(id) {
                Some(t) => dy -= t.delta().y / s,
                None => list.touch = None,
            }
        }
        let max = ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.0);
        let mut y = pos.0.y;
        if needs_reset(was_open, open, list.seen_key, list.key) {
            y = 0.0;
            list.seen_key = list.key;
        }
        if !list.qa_done
            && max > 0.0
            && let Ok(v) = std::env::var("WOW_SCROLL")
        {
            list.qa_done = true;
            dy += if v == "end" {
                max
            } else {
                v.parse().unwrap_or(0.0)
            };
        }
        let y = step(y, dy, max);
        if (pos.0.y - y).abs() > f32::EPSILON {
            pos.0.y = y;
        }
        let hint = if y + HINT_SLACK < max { HINT_TEXT } else { "" };
        let mut own_hint = false;
        for (h, mut text) in &mut hints {
            if h.0 == entity {
                own_hint = true;
                if text.0 != hint {
                    text.0 = hint.to_string();
                }
            }
        }
        if own_hint {
            continue;
        }
        // The pill sits on the bottom edge, centred; node units are physical px over the
        // window factor and UiScale.
        let more = !hint.is_empty();
        match (more, list.pill) {
            (true, None) => {
                list.pill = Some(
                    commands
                        .spawn((
                            MorePill,
                            Node {
                                position_type: PositionType::Absolute,
                                padding: UiRect::axes(px(8), px(1)),
                                border_radius: BorderRadius::all(px(8)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.85)),
                            Text::new("v more below"),
                            TextFont::from_font_size(11.0),
                            TextColor(Color::srgb(0.6, 0.66, 0.76)),
                            GlobalZIndex(48),
                            Visibility::Hidden,
                        ))
                        .id(),
                );
            }
            (false, Some(p)) => {
                commands.entity(p).despawn();
                list.pill = None;
            }
            _ => {}
        }
        if let Some(p) = list.pill
            && let Ok((mut n, mut v)) = pills.get_mut(p)
        {
            let k = node.inverse_scale_factor();
            let (c, size) = (tf.translation * k, node.size() * k);
            n.left = px(c.x - 45.0);
            n.top = px(c.y + size.y / 2.0 - 22.0);
            v.set_if_neq(Visibility::Inherited);
        }
    }
    if hover.0 != any_hover {
        hover.0 = any_hover;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_clamps_to_content() {
        assert_eq!(step(0.0, -50.0, 200.0), 0.0);
        assert_eq!(step(180.0, 50.0, 200.0), 200.0);
        assert_eq!(step(10.0, 30.0, 200.0), 40.0);
        assert_eq!(step(40.0, 30.0, 0.0), 0.0);
    }

    #[test]
    fn resets_on_reopen_or_new_key_only() {
        assert!(needs_reset(false, true, 1, 1));
        assert!(needs_reset(true, true, 1, 2));
        assert!(!needs_reset(true, true, 2, 2));
        assert!(!needs_reset(true, false, 1, 2));
    }
}
