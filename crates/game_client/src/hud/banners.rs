//! Centre-screen banners (level up, floor names, floor cleared) and IDE-style
//! notification toasts (spell learned). Banners queue so they never overprint.

use super::actionbar::spell_signature;
use super::{ACCENT, DIM, PANEL_EDGE, drop_shadow, label, panel_bg, set_text};
use crate::combat::{CombatEvent, SimReady, SimView};
use crate::floors::LoadZone;
use bevy::prelude::*;
use combat_core::Event;
use std::collections::VecDeque;

const BANNER_SECS: f32 = 2.8;
const BANNER_FADE_IN: f32 = 0.2;
const BANNER_FADE_OUT: f32 = 0.6;
const TOAST_SECS: f32 = 6.0;
const TOAST_W: f32 = 340.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);

/// A big centred title with a subtitle, shown for a few seconds.
#[derive(Message, Clone)]
pub struct Banner {
    pub title: String,
    pub subtitle: String,
    pub color: Color,
}

#[derive(Resource, Default)]
struct BannerQueue {
    queue: VecDeque<Banner>,
    current: Option<(Banner, f32)>,
}

#[derive(Component)]
struct BannerRoot;
#[derive(Component)]
struct BannerTitle;
#[derive(Component)]
struct BannerSubtitle;
#[derive(Component)]
struct ToastStack;
#[derive(Component)]
struct Toast {
    age: f32,
}

pub struct BannersPlugin;

impl Plugin for BannersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Banner>()
            .init_resource::<BannerQueue>()
            .add_systems(Startup, spawn_banner_ui)
            .add_systems(
                Update,
                (progress_events, show_banner, age_toasts, place_toasts)
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// Touch keeps the bottom-right corner for the thumb cluster: notifications stack above the
/// action bar in the centre instead, and fade on their own.
fn place_toasts(
    touch: Res<crate::touch::TouchMode>,
    mut stack: Single<&mut Node, With<ToastStack>>,
) {
    if !touch.is_changed() {
        return;
    }
    let (left, right, margin, bottom) = if touch.active {
        (
            percent(50),
            Val::Auto,
            UiRect::left(px(-TOAST_W / 2.0)),
            px(156),
        )
    } else {
        (Val::Auto, px(16), UiRect::ZERO, px(40))
    };
    stack.left = left;
    stack.right = right;
    stack.margin = margin;
    stack.bottom = bottom;
}

fn spawn_banner_ui(mut commands: Commands) {
    commands
        .spawn((
            BannerRoot,
            Node {
                position_type: PositionType::Absolute,
                top: percent(20),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(58),
        ))
        .with_children(|c| {
            c.spawn((
                BannerTitle,
                label("", 40.0),
                TextColor(GOLD),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn((
                Node {
                    width: px(260),
                    height: px(2),
                    ..default()
                },
                BackgroundColor(ACCENT.with_alpha(0.6)),
            ));
            c.spawn((
                BannerSubtitle,
                label("", 16.0),
                TextColor(Color::srgb(0.88, 0.9, 0.96)),
                TextLayout::justify(Justify::Center),
            ));
        });
    // IDE notifications stack in the bottom-right corner, newest at the bottom.
    commands.spawn((
        ToastStack,
        Node {
            position_type: PositionType::Absolute,
            right: px(16),
            bottom: px(40),
            width: px(TOAST_W),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        },
        GlobalZIndex(40),
    ));
}

/// Level-ups and learned spells straight from the sim.
fn progress_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageReader<Banner>,
    mut zones: MessageReader<LoadZone>,
    mut queue: ResMut<BannerQueue>,
    sim: SimView,
    stack: Single<Entity, With<ToastStack>>,
) {
    // The banner on screen belongs to the zone just left (its name would print over the new one).
    if zones.read().count() > 0 {
        queue.current = None;
    }
    let me = sim.player();
    for b in banners.read() {
        queue.queue.push_back(b.clone());
    }
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::LevelUp { unit, level } if Some(*unit) == me => {
                queue.queue.push_back(Banner {
                    title: "LEVEL UP".into(),
                    subtitle: format!("Level {level}. New responsibilities, same salary."),
                    color: GOLD,
                });
            }
            Event::SpellLearned { unit, spell } if Some(*unit) == me => {
                let Some(def) = sim.content().spell(*spell) else {
                    continue;
                };
                let (name, sig) = spell_signature(def, sim.content());
                commands
                    .entity(*stack)
                    .with_child(toast(&def.name, &name, &sig));
            }
            Event::Death { unit, .. } if sim.unit(*unit).is_some_and(|u| u.key == "ceo") => {
                queue.queue.push_back(Banner {
                    title: "CEO DEFEATED".into(),
                    subtitle:
                        "The golden parachute deployed anyway. Press release: 'a bold new chapter.'"
                            .into(),
                    color: GOLD,
                });
            }
            _ => {}
        }
    }
}

/// "New spell: git blame" over a highlighted `fn git_blame(target) -> Dot`.
fn toast(display: &str, name: &str, sig: &str) -> impl Bundle {
    let span = |text: &str, color: Color| {
        (
            TextSpan::new(text),
            TextFont::from_font_size(14.0),
            TextColor(color),
        )
    };
    (
        Toast { age: 0.0 },
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: UiRect::new(px(12), px(10), px(8), px(8)),
            border: UiRect {
                left: px(3),
                ..UiRect::all(px(1))
            },
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor {
            left: ACCENT,
            ..BorderColor::all(PANEL_EDGE)
        },
        panel_bg(),
        drop_shadow(),
        children![
            (label("// New spell learned", 11.0), TextColor(DIM)),
            (
                label(format!("New spell: {display}"), 15.0),
                TextColor(Color::WHITE)
            ),
            (
                label("fn ", 14.0),
                TextColor(Color::srgb(0.78, 0.55, 0.95)),
                children![span(name, Color::srgb(0.95, 0.85, 0.55)), span(sig, DIM),],
            ),
        ],
    )
}

fn show_banner(
    time: Res<Time>,
    mut queue: ResMut<BannerQueue>,
    mut root: Single<&mut Node, With<BannerRoot>>,
    mut title: Single<(&mut Text, &mut TextColor), (With<BannerTitle>, Without<BannerSubtitle>)>,
    mut subtitle: Single<(&mut Text, &mut TextColor), (With<BannerSubtitle>, Without<BannerTitle>)>,
) {
    if queue.current.is_none() {
        queue.current = queue.queue.pop_front().map(|b| (b, 0.0));
    }
    let Some((banner, age)) = queue.current.as_mut() else {
        root.display = Display::None;
        return;
    };
    *age += time.delta_secs();
    let t = *age;
    let alpha = (t / BANNER_FADE_IN)
        .min((BANNER_SECS - t) / BANNER_FADE_OUT)
        .clamp(0.0, 1.0);
    root.display = Display::Flex;
    set_text(&mut title.0, &banner.title);
    title.1.0 = banner.color.with_alpha(alpha);
    set_text(&mut subtitle.0, &banner.subtitle);
    subtitle.1.0 = Color::srgba(0.88, 0.9, 0.96, alpha);
    if t >= BANNER_SECS {
        queue.current = None;
    }
}

fn age_toasts(mut commands: Commands, time: Res<Time>, mut toasts: Query<(Entity, &mut Toast)>) {
    for (e, mut toast) in &mut toasts {
        toast.age += time.delta_secs();
        if toast.age >= TOAST_SECS {
            commands.entity(e).despawn();
        }
    }
}
