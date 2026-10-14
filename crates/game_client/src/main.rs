mod anim;
mod arena;
mod audio;
mod camera;
mod catalog;
mod classes;
mod collision;
mod combat;
mod combat_anim;
mod config;
mod copy;
mod debug;
mod economy;
mod floating_text;
mod floors;
mod game;
mod gear;
mod hair;
mod hud;
mod input;
mod keybinds;
mod loot_fx;
mod nameplates;
mod net;
mod office;
mod player;
mod presentation;
mod rigs;
mod spell_fx;
mod targeting;
mod telegraph_fx;
mod touch;
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
        resolution: touch::initial_resolution(),
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
            floors::FloorsPlugin,
            input::InputPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            units::UnitsPlugin,
            targeting::TargetingPlugin,
            hud::HudPlugin,
            anim::AnimPlugin,
            combat::CombatPlugin,
            vfx::VfxPlugin,
            spell_fx::SpellFxPlugin,
            presentation::PresentationPlugin,
            floating_text::FloatingTextPlugin,
        ))
        .add_plugins((
            game::GamePlugin,
            hair::HairPlugin,
            gear::GearPlugin,
            nameplates::NameplatePlugin,
            debug::DebugPlugin,
            audio::GameAudioPlugin,
            loot_fx::LootFxPlugin,
            rigs::RigsPlugin,
            yell::YellPlugin,
        ))
        .add_plugins((
            net::NetPlugin,
            touch::TouchPlugin,
            classes::ClassesPlugin,
            keybinds::KeybindsPlugin,
            telegraph_fx::TelegraphFxPlugin,
            economy::EconomyPlugin,
            office::OfficePlugin,
        ))
        .run();
}

/// HUD sizes are authored for a 900 px tall window; scale up on bigger screens.
fn scale_ui(window: Single<&Window>, touch: Res<touch::TouchMode>, mut ui_scale: ResMut<UiScale>) {
    // Width matters too: the combat log and the centred action bar collide on narrow windows.
    let (w, h) = if debug::shooting() {
        let (w, h) = debug::shot_size();
        (w as f32, h as f32)
    } else {
        (window.width(), window.height())
    };
    // Phones: the combat log is hidden, so the bar is the only wide element and the HUD can be
    // scaled for a ~500 px tall landscape viewport, which keeps keycaps near finger size.
    let (ref_h, ref_w, min) = if touch.active {
        (
            config::TOUCH_UI_REFERENCE_HEIGHT,
            config::TOUCH_UI_REFERENCE_WIDTH,
            config::TOUCH_UI_MIN_SCALE,
        )
    } else {
        (
            config::UI_REFERENCE_HEIGHT,
            config::UI_REFERENCE_WIDTH,
            config::UI_MIN_SCALE,
        )
    };
    let scale = (h / ref_h).min(w / ref_w).clamp(min, config::UI_MAX_SCALE);
    if (ui_scale.0 - scale).abs() > 0.01 {
        ui_scale.0 = scale;
    }
}
