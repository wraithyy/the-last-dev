mod anim;
mod arena;
mod audio;
mod calendar;
mod camera;
mod catalog;
mod classes;
mod collision;
mod combat;
mod combat_anim;
mod config;
mod copy;
mod cutscene;
mod debug;
mod economy;
mod floating_text;
mod floors;
mod game;
mod gear;
mod hair;
mod hud;
mod ilmari_end;
mod ilmari_fx;
mod ilmari_open;
mod ilmari_wing;
mod ilmari_world;
mod input;
mod keybinds;
mod legacy;
mod live;
mod loot_fx;
mod marie_log;
mod minigame;
mod nameplates;
mod net;
mod office;
mod onboarding;
mod player;
mod presentation;
mod raid;
mod rigs;
mod roasts;
mod seat;
mod settings;
mod social;
mod spell_fx;
mod sprint;
mod stealth;
mod swarm_fx;
mod targeting;
mod telegraph_fx;
mod touch;
mod units;
mod vfx;
mod wing3_fx;
mod wing4_fx;
mod wipe;
mod yell;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use combat_core::TICK_HZ;

fn window() -> Window {
    #[cfg(not(target_arch = "wasm32"))]
    let mut resolution = touch::initial_resolution();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((w, h)) = debug::bench_resolution() {
        resolution = bevy::window::WindowResolution::new(w, h);
        // WOW_BENCH_SCALE=2 emulates a retina display on a 1x monitor.
        if let Some(f) = std::env::var("WOW_BENCH_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            resolution = resolution.with_scale_factor_override(f);
        }
    }
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
        #[cfg(not(target_arch = "wasm32"))]
        resolution,
        #[cfg(target_arch = "wasm32")]
        resolution: touch::initial_resolution(),
        // Vsync would cap every scenario at the display rate and hide the real frame cost.
        #[cfg(not(target_arch = "wasm32"))]
        present_mode: if debug::bench_spec().is_some() {
            bevy::window::PresentMode::Immediate
        } else {
            default()
        },
        ..default()
    }
}

fn main() {
    let mut app = App::new();
    // A command on an entity despawned the same frame should cost a log line, not the player's
    // session; debug builds and tests keep Bevy's panic so such bugs still surface.
    #[cfg(not(debug_assertions))]
    app.set_error_handler(bevy::ecs::error::warn);
    app.add_plugins(
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
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ) * qa_sim_speed()))
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
        stealth::StealthPlugin,
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
        sprint::SprintPlugin,
        onboarding::OnboardingPlugin,
    ))
    // After TouchPlugin: the settings default to Low on phones.
    .add_plugins((
        settings::SettingsPlugin,
        cutscene::CutscenePlugin,
        ilmari_open::IlmariOpenPlugin,
        ilmari_end::IlmariEndPlugin,
        marie_log::MarieLogPlugin,
        ilmari_world::IlmariWorldPlugin,
        ilmari_wing::IlmariWingPlugin,
        swarm_fx::SwarmFxPlugin,
        wing3_fx::Wing3FxPlugin,
        wing4_fx::Wing4FxPlugin,
        ilmari_fx::IlmariFxPlugin,
    ))
    .add_plugins(social::SocialPlugin)
    .add_plugins(roasts::RoastPlugin)
    .add_plugins(wipe::WipePlugin)
    .add_plugins(live::LivePlugin)
    .add_plugins(legacy::LegacyPlugin)
    .add_plugins(raid::RaidPlugin)
    .add_plugins(minigame::MinigamePlugin)
    .add_plugins(calendar::CalendarPlugin)
    .run();
}

/// HUD sizes are authored for a 900 px tall window; scale up on bigger screens.
fn scale_ui(
    window: Single<&Window>,
    touch: Res<touch::TouchMode>,
    settings: Res<settings::Settings>,
    mut ui_scale: ResMut<UiScale>,
) {
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
    let auto = (h / ref_h).min(w / ref_w).clamp(min, config::UI_MAX_SCALE);
    let scale = settings.ui_scale.unwrap_or(auto);
    if (ui_scale.0 - scale).abs() > 0.01 {
        ui_scale.0 = scale;
    }
}

/// QA only (offline): `WOW_SIM_SPEED=<n>` runs the sim n times faster, so a screenshot can reach
/// the late phases of a long fight. Never set online (the server's clock does not follow).
fn qa_sim_speed() -> f64 {
    std::env::var("WOW_SIM_SPEED")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|n| (1.0..=16.0).contains(n))
        .unwrap_or(1.0)
}
