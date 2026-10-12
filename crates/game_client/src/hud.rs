//! Phase 1 HUD: only values that exist in the simulation. No action bar yet.

use crate::config::*;
use crate::input::Paused;
use crate::targeting::TargetInfo;
use bevy::prelude::*;

#[derive(Component)]
struct TargetFrame;
/// Which piece of target data a text node displays.
#[derive(Component, Clone, Copy)]
enum Field {
    Name,
    Distance,
    Range,
    Facing,
    Sight,
}
#[derive(Component)]
struct PauseOverlay;

const FRAME_WIDTH: f32 = 220.0;
const PANEL: Color = Color::srgba(0.05, 0.06, 0.1, 0.75);
const GOOD: Color = Color::srgb(0.12, 0.45, 0.2);
const BAD: Color = Color::srgb(0.6, 0.15, 0.12);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_target_frame, update_pause_overlay));
    }
}

fn label(text: impl Into<String>, size: f32) -> impl Bundle {
    (Text::new(text), TextFont::from_font_size(size))
}

fn chip(kind: Field) -> impl Bundle {
    (
        kind,
        label("", 14.0),
        Node {
            padding: UiRect::axes(px(6), px(2)),
            ..default()
        },
        BackgroundColor(GOOD),
    )
}

fn spawn_hud(mut commands: Commands) {
    // Independent absolute panels (no shared root) so nothing stretches across the screen.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            width: px(FRAME_WIDTH),
            padding: UiRect::all(px(8)),
            ..default()
        },
        BackgroundColor(PANEL),
        children![label(format!("{PLAYER_NAME}  Lv {PLAYER_LEVEL}"), 18.0)],
    ));
    commands.spawn((
        TargetFrame,
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(24.0 + FRAME_WIDTH),
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL),
        children![
            (Field::Name, label("", 18.0)),
            (Field::Distance, label("", 14.0)),
            (
                Node {
                    column_gap: px(4),
                    ..default()
                },
                children![chip(Field::Range), chip(Field::Facing), chip(Field::Sight)]
            ),
        ],
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(10),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            label(
                "W/S move  A/D turn  Q/E strafe  RMB steer  LMB orbit  Wheel zoom  Space jump  Tab target  Esc clear/menu",
                13.0,
            ),
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        )],
    ));

    commands.spawn((
        PauseOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![label("Paused - press Esc to resume", 32.0)],
    ));
}

fn update_target_frame(
    info: Res<TargetInfo>,
    mut frame: Single<&mut Node, With<TargetFrame>>,
    mut fields: Query<(&Field, &mut Text, Option<&mut BackgroundColor>)>,
) {
    let Some(t) = &info.0 else {
        frame.display = Display::None;
        return;
    };
    frame.display = Display::Flex;
    for (field, mut text, bg) in &mut fields {
        // (text, chip state): None for plain labels.
        let (value, ok) = match field {
            Field::Name => (format!("{}  Lv {}", t.name, t.level), None),
            Field::Distance => (format!("{:.1} m", t.distance), None),
            Field::Range if t.in_range => ("In range".into(), Some(true)),
            Field::Range => (
                format!("Out of range ({PREVIEW_SPELL_RANGE:.0} m)"),
                Some(false),
            ),
            Field::Facing if t.facing => ("Facing".into(), Some(true)),
            Field::Facing => ("Not facing".into(), Some(false)),
            Field::Sight if t.line_of_sight => ("LoS".into(), Some(true)),
            Field::Sight => ("No line of sight".into(), Some(false)),
        };
        text.0 = value;
        if let (Some(ok), Some(mut bg)) = (ok, bg) {
            bg.0 = if ok { GOOD } else { BAD };
        }
    }
}

fn update_pause_overlay(paused: Res<Paused>, mut overlay: Single<&mut Node, With<PauseOverlay>>) {
    overlay.display = if paused.0 {
        Display::Flex
    } else {
        Display::None
    };
}
