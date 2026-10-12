//! WoW-style HUD. Every value shown here is read from the combat simulation
//! (`CombatSim`) or from a sim event; nothing is animated for show.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod actionbar;
mod feed;
mod frames;

use crate::combat::{CombatSim, SimReady};
use crate::input::Paused;
use crate::targeting::TargetInfo;
use bevy::prelude::*;
use std::collections::HashMap;

pub use actionbar::CooldownMaterial;

pub const GOLD: Color = Color::srgb(0.78, 0.62, 0.25);
pub const BRONZE: Color = Color::srgb(0.32, 0.22, 0.1);
pub const PANEL: Color = Color::srgba(0.05, 0.05, 0.09, 0.85);
pub const BAR_BG: Color = Color::srgb(0.07, 0.07, 0.1);
pub const HP_GREEN: Color = Color::srgb(0.16, 0.68, 0.22);
pub const MANA_BLUE: Color = Color::srgb(0.18, 0.38, 0.95);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconCache>()
            .init_resource::<DevOverlay>()
            .add_plugins((
                UiMaterialPlugin::<CooldownMaterial>::default(),
                frames::FramesPlugin,
                actionbar::ActionBarPlugin,
                feed::FeedPlugin,
            ))
            .add_systems(Startup, spawn_misc)
            .add_systems(
                Update,
                (update_pause_overlay, toggle_dev, update_dev_overlay).in_set(SimReady),
            );
    }
}

/// Icon handles by asset key, so per-frame UI updates never reload files.
#[derive(Resource, Default)]
pub struct IconCache(HashMap<String, Handle<Image>>);

impl IconCache {
    pub fn get(&mut self, assets: &AssetServer, key: &str) -> Handle<Image> {
        self.0
            .entry(key.to_owned())
            .or_insert_with(|| assets.load(format!("icons/{key}.png")))
            .clone()
    }
}

pub fn label(text: impl Into<String>, size: f32) -> impl Bundle {
    (Text::new(text), TextFont::from_font_size(size))
}

/// Writes only on change so Bevy does not re-layout unchanged text every frame.
pub fn set_text(text: &mut Text, value: &str) {
    if text.0 != value {
        value.clone_into(&mut text.0);
    }
}

/// "Prompt Fragment" -> "prompt_fragment": item and icon keys are snake_case.
pub fn asset_key(display_name: &str) -> String {
    display_name.to_lowercase().replace(' ', "_")
}

/// Whole seconds rounded up, WoW style ("12", then "1m" past a minute).
pub fn fmt_secs(ms: u32) -> String {
    let s = ms.div_ceil(1000);
    if s >= 60 {
        format!("{}m", s.div_ceil(60))
    } else {
        s.to_string()
    }
}

/// Thin bronze line inset inside a gold-bordered panel: the original "ornate" double border.
pub fn inset_line(radius: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(3),
            right: px(3),
            top: px(3),
            bottom: px(3),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px((radius - 3.0).max(0.0))),
            ..default()
        },
        BorderColor::all(BRONZE),
    )
}

#[derive(Component)]
struct PauseOverlay;
#[derive(Component)]
struct DevText;
#[derive(Resource, Default)]
struct DevOverlay(bool);

fn spawn_misc(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(4),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            label(
                "W/S move  A/D turn  Q/E strafe  RMB steer  LMB orbit  Wheel zoom  Space jump  Tab target  1-4 cast  F loot  F3 debug  Esc clear/menu",
                12.0,
            ),
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.45)),
        )],
    ));
    commands.spawn((
        DevText,
        Node {
            position_type: PositionType::Absolute,
            top: px(110),
            left: px(12),
            padding: UiRect::all(px(6)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL),
        label("", 13.0),
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
        GlobalZIndex(50),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![label("Paused - press Esc to resume", 32.0)],
    ));
}

fn update_pause_overlay(paused: Res<Paused>, mut overlay: Single<&mut Node, With<PauseOverlay>>) {
    overlay.display = if paused.0 {
        Display::Flex
    } else {
        Display::None
    };
}

fn toggle_dev(keys: Res<ButtonInput<KeyCode>>, mut dev: ResMut<DevOverlay>) {
    if keys.just_pressed(KeyCode::F3) {
        dev.0 = !dev.0;
    }
}

/// Phase-1 geometry readout (range / facing / line of sight), kept for debugging only.
fn update_dev_overlay(
    dev: Res<DevOverlay>,
    sim: Res<CombatSim>,
    info: Res<TargetInfo>,
    mut text: Single<(&mut Node, &mut Text), With<DevText>>,
) {
    let (node, text) = &mut *text;
    node.display = if dev.0 { Display::Flex } else { Display::None };
    if !dev.0 {
        return;
    }
    let yn = |b: bool| if b { "ok" } else { "NO" };
    let line = match &info.0 {
        Some(t) => format!(
            "tick {:?}\ntarget {:.1} m (spell range {:.0} m)\nrange {}  facing {}  LoS {}",
            sim.0.now(),
            t.distance,
            t.spell_range,
            yn(t.in_range),
            yn(t.facing),
            yn(t.line_of_sight),
        ),
        None => format!("tick {:?}\nno target", sim.0.now()),
    };
    set_text(text, &line);
}
