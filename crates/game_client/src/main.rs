mod anim;
mod arena;
mod camera;
mod collision;
mod combat;
mod combat_anim;
mod config;
mod copy;
mod debug;
mod floating_text;
mod hud;
mod input;
mod player;
mod presentation;
mod targeting;
mod units;
mod vfx;
mod yell;

use bevy::asset::AssetMetaCheck;
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
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window()),
                    ..default()
                })
                // We ship no .meta files; probing for them costs a 404 per asset on the web.
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .add_systems(Update, scale_ui)
        .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
        .add_plugins((
            arena::ArenaPlugin,
            input::InputPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            units::UnitsPlugin,
            targeting::TargetingPlugin,
            hud::HudPlugin,
            anim::AnimPlugin,
            combat::CombatPlugin,
            vfx::VfxPlugin,
            presentation::PresentationPlugin,
            floating_text::FloatingTextPlugin,
            yell::YellPlugin,
            debug::DebugPlugin,
        ))
        .run();
}

/// HUD sizes are authored for a 900 px tall window; scale up on bigger screens.
fn scale_ui(window: Single<&Window, Changed<Window>>, mut ui_scale: ResMut<UiScale>) {
    let scale = (window.height() / config::UI_REFERENCE_HEIGHT).clamp(1.0, config::UI_MAX_SCALE);
    if (ui_scale.0 - scale).abs() > 0.01 {
        ui_scale.0 = scale;
    }
}
