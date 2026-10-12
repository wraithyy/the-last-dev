mod arena;
mod camera;
mod collision;
mod config;
mod hud;
mod input;
mod player;
mod targeting;
mod units;

use bevy::prelude::*;
use bevy::window::WindowPlugin;
use combat_core::TICK_HZ;

fn window() -> Window {
    Window {
        title: config::WINDOW_TITLE.into(),
        // Browser-only settings: embed in the page canvas, and keep right-click /
        // wheel from triggering the context menu or page scroll during play.
        #[cfg(target_arch = "wasm32")]
        canvas: Some("#bevy".into()),
        #[cfg(target_arch = "wasm32")]
        fit_canvas_to_parent: true,
        #[cfg(target_arch = "wasm32")]
        prevent_default_event_handling: true,
        ..default()
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(window()),
            ..default()
        }))
        .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
        .add_plugins((
            arena::ArenaPlugin,
            input::InputPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            units::UnitsPlugin,
            targeting::TargetingPlugin,
            hud::HudPlugin,
        ))
        .run();
}
