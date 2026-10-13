//! Developer-flavoured HUD (keycaps, terminal log, IDE popups). Every value shown here is read from the combat simulation
//! (`CombatSim`) or from a sim event; nothing is animated for show.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod actionbar;
mod banners;
mod feed;
mod frames;
mod map;
mod quests;
mod trophies;

use crate::combat::{SimReady, SimView};
use crate::input::Paused;
use crate::targeting::TargetInfo;
use bevy::prelude::*;
use std::collections::HashMap;

pub use actionbar::CooldownMaterial;
pub use banners::Banner;
pub use feed::{CombatLogRoot, LogLineMsg};
pub use quests::giver_mark;

/// Terminal cyan: focus, hover and "active" states across the HUD.
pub const ACCENT: Color = Color::srgb(0.36, 0.82, 0.86);
/// Warm orange for the auto attack keycap, like an artisan Esc/Enter key.
pub const ACCENT_WARM: Color = Color::srgb(1.0, 0.55, 0.25);
pub const KEY_TOP: Color = Color::srgb(0.2, 0.22, 0.28);
pub const KEY_TOP_HI: Color = Color::srgb(0.3, 0.33, 0.41);
pub const KEY_SIDE: Color = Color::srgb(0.06, 0.07, 0.1);
pub const KEY_RIM: Color = Color::srgb(0.4, 0.44, 0.54);
/// Editor chrome: popups, loot window, log body.
pub const PANEL: Color = Color::srgba(0.05, 0.06, 0.09, 0.88);
pub const PANEL_TOP: Color = Color::srgba(0.1, 0.11, 0.15, 0.94);
pub const PANEL_BOTTOM: Color = Color::srgba(0.04, 0.045, 0.07, 0.94);
pub const PANEL_EDGE: Color = Color::srgb(0.26, 0.3, 0.38);
pub const BAR_BG: Color = Color::srgb(0.07, 0.07, 0.1);
pub const DIM: Color = Color::srgb(0.6, 0.66, 0.76);
pub const HP_GREEN: Color = Color::srgb(0.16, 0.68, 0.22);
pub const MANA_BLUE: Color = Color::srgb(0.18, 0.38, 0.95);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconCache>()
            .add_systems(PreStartup, load_font)
            .add_systems(Update, apply_font)
            .init_resource::<DevOverlay>()
            .add_plugins((
                UiMaterialPlugin::<CooldownMaterial>::default(),
                frames::FramesPlugin,
                actionbar::ActionBarPlugin,
                feed::FeedPlugin,
                banners::BannersPlugin,
                trophies::TrophiesPlugin,
                quests::QuestsPlugin,
                map::MapPlugin,
            ))
            .add_systems(Startup, spawn_misc)
            .add_systems(
                Update,
                (update_pause_overlay, toggle_dev, update_dev_overlay).in_set(SimReady),
            );
    }
}

/// Icon files shipped in `assets/icons` (see CREDITS.md). Any other key gets a
/// generated letter badge, so content can name icons before art exists.
pub const ICON_FILES: [&str; 160] = [
    "ab_loser",
    "ab_test",
    "ab_winner",
    "accepted",
    "access_badge",
    "agent_memory",
    "agentic_copy",
    "agi_pivot",
    "alignment_training",
    "analysis",
    "backlog",
    "badge",
    "batch_blessing",
    "benchmark",
    "big_diff",
    "billable_hours",
    "block_all",
    "box",
    "brain",
    "brand",
    "brand_awareness",
    "caffeine",
    "calendar",
    "cash",
    "chart",
    "chart_up",
    "chat_bubble",
    "chatbot_rollout",
    "code_review",
    "code_smell",
    "coffee_break",
    "coins",
    "confident_answer",
    "context_window",
    "contract",
    "cron",
    "culture_fit",
    "deadline",
    "delivery_receipt",
    "demo_crashed",
    "demo_lanyard",
    "demo_laptop",
    "demo_works",
    "emergent_capability",
    "ethics_review",
    "exhaust_gust",
    "expired_gpu_credits",
    "fair_share",
    "family",
    "fan_blade",
    "forwarded_reply",
    "free_swag",
    "generate_prs",
    "git_blame",
    "gpu_candle",
    "guardrails",
    "hallucinate_copies",
    "hallucinated_hotfix",
    "hallucination_blast",
    "hit_and_run",
    "hook",
    "hot_aisle",
    "hourglass",
    "human_inside",
    "incident_report",
    "influencer_collab",
    "jailbreak",
    "job_pending",
    "judgment",
    "key",
    "keyboard",
    "lighter",
    "live_demo",
    "locked_out",
    "logo",
    "mailbox",
    "mandatory_prompt_training_certificate",
    "manifesto",
    "matrix",
    "medal",
    "megaphone",
    "melee",
    "nitpick",
    "oat_milk_prompt",
    "org_chart",
    "overheating",
    "overpromise",
    "padlock",
    "parachute",
    "password_rotation",
    "phished",
    "phishing",
    "pizza",
    "postmortem",
    "preemption",
    "press_release",
    "priority_low",
    "prompt_certificate",
    "prompt_fragment",
    "prompt_injected",
    "pto",
    "pull_request",
    "queue_ticket",
    "quick_sync",
    "rack_chair",
    "rack_failover",
    "rack_screw",
    "rebrand",
    "rebranded",
    "recalculating_route",
    "refactor",
    "regen",
    "reject_pr",
    "relationship_building",
    "reorg",
    "reprioritize",
    "requirements_change",
    "retrospective",
    "review_fatigue",
    "review_request",
    "robot",
    "robot_audit",
    "rollback",
    "room_booking",
    "rss",
    "rubber_duck",
    "rug_pull",
    "scooter_helmet",
    "scope_creep",
    "self_improvement",
    "series_b",
    "soc",
    "spiral",
    "stakeholders",
    "standup",
    "sticky",
    "still_typing",
    "stress_ball",
    "synergy_storm",
    "term_sheet",
    "tests_off",
    "thermal_event",
    "thermal_paste",
    "thought_leadership",
    "thumb_down",
    "token_invoice",
    "unread_ai_strategy_deck",
    "unread_spec",
    "vibes",
    "viral_loop",
    "visitor_badge",
    "webinar",
    "wellness",
    "wellness_survey",
    "wont_fix",
    "write_by_hand",
    "wrong_thread",
    "yesterday",
    "zero_trust",
    "zip_file",
];

/// Icon handles by asset key, so per-frame UI updates never reload files.
#[derive(Resource, Default)]
pub struct IconCache(HashMap<String, Handle<Image>>);

impl IconCache {
    pub fn get(&mut self, assets: &AssetServer, key: &str) -> Handle<Image> {
        self.0
            .entry(key.to_owned())
            .or_insert_with(|| {
                if ICON_FILES.contains(&key) {
                    assets.load(format!("icons/{key}.png"))
                } else {
                    assets.add(crate::floors::icon_image(key))
                }
            })
            .clone()
    }
}

/// Every HUD label carries a hard drop shadow: readable over both the dark panels and the bright 3D scene.
pub fn label(text: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.9),
        },
    )
}

#[derive(Resource)]
struct HudFont(Handle<Font>);

fn load_font(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(HudFont(assets.load("fonts/JetBrainsMono-Bold.ttf")));
}

/// One place swaps the font for every text node (HUD, floating text, bubbles) instead of threading a handle through each spawn.
fn apply_font(font: Res<HudFont>, mut fonts: Query<&mut TextFont, Added<TextFont>>) {
    for mut f in &mut fonts {
        f.font = bevy::text::FontSource::Handle(font.0.clone());
    }
}

/// Vertical two-stop gradient background (top to bottom).
pub fn vgrad(top: Color, bottom: Color) -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(top, percent(0)),
        ColorStop::new(bottom, percent(100)),
    ]))
}

pub fn panel_bg() -> BackgroundGradient {
    vgrad(PANEL_TOP, PANEL_BOTTOM)
}

pub fn drop_shadow() -> BoxShadow {
    BoxShadow::new(
        Color::srgba(0.0, 0.0, 0.0, 0.55),
        px(0),
        px(3),
        px(0),
        px(8),
    )
}

/// Glass highlight over the upper half of a bar or slot.
pub fn gloss(alpha: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            height: percent(46),
            ..default()
        },
        vgrad(
            Color::srgba(1.0, 1.0, 1.0, alpha),
            Color::srgba(1.0, 1.0, 1.0, 0.0),
        ),
    )
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

#[derive(Component)]
struct PauseOverlay;
#[derive(Component)]
struct DevText;
#[derive(Resource, Default)]
struct DevOverlay(bool);

/// Keyboard cheat sheet along the top edge; hidden on touch layouts.
#[derive(Component)]
pub struct KeyHint;

fn spawn_misc(mut commands: Commands) {
    commands.spawn((
        KeyHint,
        Node {
            position_type: PositionType::Absolute,
            top: px(4),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            label(
                "WASD move | RMB steer | LMB orbit | Wheel zoom | Space jump | Tab target | T auto | 1-0 cast | F / right-click talk, loot | J jira | N map | B trophies | M mute | Esc menu",
                11.0,
            ),
            TextColor(Color::srgba(0.8, 0.9, 1.0, 0.5)),
        )],
    ));
    commands.spawn((
        DevText,
        Node {
            position_type: PositionType::Absolute,
            top: px(150),
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
        children![label(
            "// paused: the AI is regenerating your sprint. Esc to resume.",
            24.0
        )],
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
    sim: SimView,
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
            sim.now(),
            t.distance,
            t.spell_range,
            yn(t.in_range),
            yn(t.facing),
            yn(t.line_of_sight),
        ),
        None => format!("tick {:?}\nno target", sim.now()),
    };
    set_text(text, &line);
}

#[cfg(test)]
mod tests {
    use super::ICON_FILES;

    /// Anything without a shipped file renders as a generated letter badge.
    #[test]
    fn every_spell_aura_and_trophy_has_a_shipped_icon_file() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        let c = sim.content();
        let icons = c
            .items
            .iter()
            .map(|i| (&i.name, &i.icon))
            .chain(c.spells.iter().map(|s| (&s.name, &s.icon)))
            .chain(c.auras.iter().map(|a| (&a.name, &a.icon)));
        let missing: Vec<String> = icons
            .filter(|(_, icon)| !ICON_FILES.contains(&icon.as_str()))
            .map(|(name, icon)| format!("{name}: {icon}"))
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    /// WebGL2 has no 16-bit normalized textures: one such PNG makes the browser build quit on load.
    #[test]
    fn every_png_asset_is_8_bit() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "png") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
            &mut files,
        );
        assert!(!files.is_empty());
        for f in files {
            let bytes = std::fs::read(&f).unwrap();
            // IHDR bit depth sits at byte 24 of every PNG.
            assert_eq!(bytes.get(24), Some(&8), "{} is not 8-bit", f.display());
        }
    }

    #[test]
    fn listed_icon_files_exist() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
        for k in ICON_FILES {
            assert!(dir.join(format!("{k}.png")).exists(), "{k}");
        }
    }
}

/// QA hook (`WOW_HOVER_SLOT`): forces the tooltip of one action slot so it can be screenshotted without a pointer.
#[derive(Resource)]
pub struct DebugHoverSlot(pub usize);
