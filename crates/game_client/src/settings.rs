//! `~/settings` (Esc menu): quality preset, field of view, UI scale, colourblind palettes,
//! reduce motion and an FPS readout. Stored per viewer (a native file next to the keybinds, or
//! localStorage on the web); phones default to the Low preset. Everything stays phone-safe:
//! one shadow cascade, bloom on or off.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::camera::OrbitCamera;
use crate::hud::{
    ACCENT, ACCENT_WARM, BAR_BG, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, set_text,
    vgrad,
};
use crate::input::Paused;
use crate::net::Lobby;
use crate::touch::TouchMode;
use bevy::camera::visibility::VisibilityRange;
use bevy::input::InputSystems;
use bevy::light::{CascadeShadowConfig, DirectionalLightShadowMap};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

pub const FOV_MIN: f32 = 40.0;
pub const FOV_MAX: f32 = 90.0;
/// Bevy's default perspective (45 degrees vertical), which the game was tuned with.
pub const FOV_DEFAULT: f32 = 45.0;
pub const UI_SCALE_MIN: f32 = 0.6;
pub const UI_SCALE_MAX: f32 = 2.0;
pub const RENDER_SCALE_MIN: f32 = 0.5;
pub const RENDER_SCALE_MAX: f32 = 1.0;
pub const VIEW_DIST_MIN: f32 = 0.4;
pub const VIEW_DIST_MAX: f32 = 1.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
}

impl Quality {
    pub const ALL: [Quality; 4] = [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra];

    pub fn key(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
            Quality::Ultra => "ultra",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
            Quality::Ultra => "Ultra",
        }
    }

    /// One preset cheaper, if there is one.
    pub fn lower(self) -> Option<Quality> {
        let i = Quality::ALL.iter().position(|q| *q == self)?;
        i.checked_sub(1).map(|i| Quality::ALL[i])
    }

    pub fn shadows(self) -> bool {
        self != Quality::Low
    }

    pub fn shadow_map(self) -> usize {
        match self {
            Quality::Low | Quality::Medium => 1024,
            Quality::High => 2048,
            Quality::Ultra => 4096,
        }
    }

    /// Far edge of the single shadow cascade, metres.
    pub fn shadow_range(self) -> f32 {
        match self {
            Quality::Low | Quality::Medium => 40.0,
            Quality::High => 60.0,
            Quality::Ultra => 90.0,
        }
    }

    pub fn bloom(self) -> bool {
        self != Quality::Low
    }

    /// 4x only: wgpu guarantees 1 and 4 samples on every backend, so a 2x step would not be portable.
    pub fn msaa(self) -> bool {
        matches!(self, Quality::High | Quality::Ultra)
    }

    /// Share of the physical resolution the 3D view is rendered at (browser only, see
    /// `apply_render_scale`).
    pub fn render_scale(self) -> f32 {
        match self {
            Quality::Low => 0.6,
            Quality::Medium => 0.75,
            Quality::High => 0.9,
            Quality::Ultra => 1.0,
        }
    }

    /// Share of the authored decor view distance.
    pub fn decor_range(self) -> f32 {
        match self {
            Quality::Low => 0.6,
            Quality::Medium => 0.8,
            Quality::High => 1.0,
            Quality::Ultra => 1.3,
        }
    }

    /// Metres within which NPCs are animated and drawn; for the unit systems to read through
    /// `Settings::npc_range`.
    pub fn npc_range(self) -> f32 {
        match self {
            Quality::Low => 45.0,
            Quality::Medium => 60.0,
            Quality::High => 80.0,
            Quality::Ultra => 110.0,
        }
    }

    /// Most point lights worth lighting at once (keeps the low end cheap).
    #[allow(dead_code)] // consumed by the render round's light, water and particle code
    pub fn max_lights(self) -> usize {
        match self {
            Quality::Low => 2,
            Quality::Medium => 4,
            Quality::High => 8,
            Quality::Ultra => 16,
        }
    }

    /// 0 flat colour, 1 animated surface, 2 animated with highlights.
    #[allow(dead_code)] // consumed by the render round's light, water and particle code
    pub fn water_level(self) -> u8 {
        match self {
            Quality::Low => 0,
            Quality::Medium => 1,
            Quality::High | Quality::Ultra => 2,
        }
    }

    /// Multiplier on how many particles an effect spawns.
    #[allow(dead_code)] // consumed by the render round's light, water and particle code
    pub fn particles(self) -> f32 {
        match self {
            Quality::Low => 0.35,
            Quality::Medium => 0.65,
            Quality::High | Quality::Ultra => 1.0,
        }
    }
}

/// How much of the `~/perf $ top` window shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PerfMode {
    Off,
    Compact,
    Full,
}

impl PerfMode {
    pub const ALL: [PerfMode; 3] = [PerfMode::Off, PerfMode::Compact, PerfMode::Full];

    fn key(self) -> &'static str {
        match self {
            PerfMode::Off => "off",
            PerfMode::Compact => "compact",
            PerfMode::Full => "full",
        }
    }

    pub fn next(self) -> PerfMode {
        match self {
            PerfMode::Off => PerfMode::Compact,
            PerfMode::Compact => PerfMode::Full,
            PerfMode::Full => PerfMode::Off,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colorblind {
    Off,
    Deuteranopia,
    Protanopia,
    Tritanopia,
}

impl Colorblind {
    pub const ALL: [Colorblind; 4] = [
        Colorblind::Off,
        Colorblind::Deuteranopia,
        Colorblind::Protanopia,
        Colorblind::Tritanopia,
    ];

    fn key(self) -> &'static str {
        match self {
            Colorblind::Off => "off",
            Colorblind::Deuteranopia => "deuteranopia",
            Colorblind::Protanopia => "protanopia",
            Colorblind::Tritanopia => "tritanopia",
        }
    }
}

#[derive(Resource, Clone, PartialEq, Debug)]
pub struct Settings {
    pub quality: Quality,
    pub fov_deg: f32,
    /// `None` follows the window size (the HUD's own auto scale).
    pub ui_scale: Option<f32>,
    pub colorblind: Colorblind,
    /// No camera shake, no spell flashes, no cutscene camera drift.
    pub reduce_motion: bool,
    pub perf: PerfMode,
    /// Advanced overrides of single preset values; `None` follows the preset.
    pub render_scale: Option<f32>,
    pub msaa: Option<bool>,
    pub shadows: Option<bool>,
    /// Share of the authored view distance (decor and NPCs).
    pub view_dist: Option<f32>,
    /// A corpse loots fully on open and the loot window only appears for what did not fit.
    pub auto_loot: bool,
}

impl Settings {
    pub fn defaults(mobile: bool) -> Self {
        Settings {
            quality: if mobile { Quality::Low } else { Quality::High },
            fov_deg: FOV_DEFAULT,
            ui_scale: None,
            colorblind: Colorblind::Off,
            reduce_motion: false,
            perf: PerfMode::Off,
            render_scale: None,
            msaa: None,
            shadows: None,
            view_dist: None,
            auto_loot: mobile,
        }
    }

    pub fn shadows(&self) -> bool {
        self.shadows.unwrap_or_else(|| self.quality.shadows())
    }

    pub fn msaa(&self) -> bool {
        self.msaa.unwrap_or_else(|| self.quality.msaa())
    }

    pub fn render_scale(&self) -> f32 {
        self.render_scale
            .unwrap_or_else(|| self.quality.render_scale())
    }

    /// Share of the authored decor view distance.
    pub fn decor_range(&self) -> f32 {
        self.view_dist.unwrap_or_else(|| self.quality.decor_range())
    }

    /// Metres within which NPCs are worth animating and drawing.
    #[allow(dead_code)] // read by the unit systems of the render round
    pub fn npc_range(&self) -> f32 {
        self.quality.npc_range() * self.decor_range() / self.quality.decor_range()
    }

    /// A preset pick drops the single-value overrides.
    pub fn set_quality(&mut self, q: Quality) {
        self.quality = q;
        self.render_scale = None;
        self.msaa = None;
        self.shadows = None;
        self.view_dist = None;
    }

    pub fn encode(&self) -> String {
        let opt = |v: Option<String>| v.unwrap_or_else(|| "auto".into());
        format!(
            "quality={}\nfov={:.0}\nui_scale={}\ncolorblind={}\nreduce_motion={}\nperf={}\nauto_loot={}\nrender_scale={}\nmsaa={}\nshadows={}\nview_dist={}\n",
            self.quality.key(),
            self.fov_deg,
            self.ui_scale.map_or("auto".into(), |s| format!("{s:.2}")),
            self.colorblind.key(),
            u8::from(self.reduce_motion),
            self.perf.key(),
            u8::from(self.auto_loot),
            opt(self.render_scale.map(|v| format!("{v:.2}"))),
            opt(self.msaa.map(|v| u8::from(v).to_string())),
            opt(self.shadows.map(|v| u8::from(v).to_string())),
            opt(self.view_dist.map(|v| format!("{v:.2}"))),
        )
    }

    /// Unknown keys and bad values keep the platform default; numbers are clamped.
    pub fn decode(text: &str, mobile: bool) -> Self {
        let mut s = Settings::defaults(mobile);
        let num = |v: &str, lo: f32, hi: f32| {
            v.parse::<f32>()
                .ok()
                .filter(|f| f.is_finite())
                .map(|f| f.clamp(lo, hi))
        };
        let flag = |v: &str| match v {
            "1" => Some(true),
            "0" => Some(false),
            _ => None,
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "quality" => {
                    if let Some(q) = Quality::ALL.into_iter().find(|q| q.key() == v) {
                        s.quality = q;
                    }
                }
                "fov" => {
                    if let Ok(f) = v.parse::<f32>().map(|f| f.clamp(FOV_MIN, FOV_MAX))
                        && f.is_finite()
                    {
                        s.fov_deg = f;
                    }
                }
                "ui_scale" => {
                    s.ui_scale = v
                        .parse::<f32>()
                        .ok()
                        .filter(|f| f.is_finite())
                        .map(|f| f.clamp(UI_SCALE_MIN, UI_SCALE_MAX));
                }
                "colorblind" => {
                    if let Some(c) = Colorblind::ALL.into_iter().find(|c| c.key() == v) {
                        s.colorblind = c;
                    }
                }
                "reduce_motion" => s.reduce_motion = v == "1",
                // Older files stored a plain FPS toggle.
                "show_fps" if v == "1" => s.perf = PerfMode::Compact,
                "perf" => {
                    if let Some(m) = PerfMode::ALL.into_iter().find(|m| m.key() == v) {
                        s.perf = m;
                    }
                }
                "render_scale" => s.render_scale = num(v, RENDER_SCALE_MIN, RENDER_SCALE_MAX),
                "view_dist" => s.view_dist = num(v, VIEW_DIST_MIN, VIEW_DIST_MAX),
                "msaa" => s.msaa = flag(v),
                "shadows" => s.shadows = flag(v),
                "auto_loot" => s.auto_loot = v == "1",
                _ => {}
            }
        }
        s
    }
}

/// Colours that carry meaning (who is hostile, how hurt, where not to stand), per palette.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    pub hostile: Color,
    pub friendly: Color,
    /// Player and party HP bars.
    pub health: Color,
    /// Enemy nameplate HP bars.
    pub enemy_health: Color,
    /// Ground telegraphs to leave, and the safe spots inside them.
    pub danger: [f32; 3],
    pub safe: [f32; 3],
}

impl Default for Palette {
    fn default() -> Self {
        palette(Colorblind::Off)
    }
}

/// Red/green pairs become orange/blue for the red-green types (protanopes see dark reds as
/// black, so theirs are brighter), and red/teal for tritanopes, who confuse blue and yellow.
pub fn palette(c: Colorblind) -> Palette {
    let rgb = Color::srgb;
    match c {
        Colorblind::Off => Palette {
            hostile: rgb(1.0, 0.3, 0.25),
            friendly: rgb(0.4, 1.0, 0.45),
            health: crate::hud::HP_GREEN,
            enemy_health: rgb(0.85, 0.15, 0.12),
            danger: [1.0, 0.28, 0.06],
            safe: [0.25, 0.95, 0.45],
        },
        Colorblind::Deuteranopia => Palette {
            hostile: rgb(1.0, 0.55, 0.1),
            friendly: rgb(0.35, 0.7, 1.0),
            health: rgb(0.2, 0.55, 1.0),
            enemy_health: rgb(0.95, 0.5, 0.05),
            danger: [0.95, 0.25, 0.85],
            safe: [0.3, 0.6, 1.0],
        },
        Colorblind::Protanopia => Palette {
            hostile: rgb(1.0, 0.75, 0.15),
            friendly: rgb(0.4, 0.75, 1.0),
            health: rgb(0.25, 0.6, 1.0),
            enemy_health: rgb(1.0, 0.7, 0.1),
            danger: [1.0, 0.4, 0.95],
            safe: [0.35, 0.65, 1.0],
        },
        Colorblind::Tritanopia => Palette {
            hostile: rgb(1.0, 0.25, 0.35),
            friendly: rgb(0.2, 0.9, 0.85),
            health: rgb(0.1, 0.75, 0.7),
            enemy_health: rgb(0.9, 0.15, 0.3),
            danger: [1.0, 0.15, 0.3],
            safe: [0.2, 0.9, 0.85],
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ctl {
    Open,
    Close,
    Reset,
    Quality(Quality),
    Colorblind(Colorblind),
    Fov(i8),
    Ui(i8),
    UiAuto,
    Render(i8),
    View(i8),
    Msaa,
    Shadows,
    Motion,
    Perf(PerfMode),
    AutoLoot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SliderKind {
    Fov,
    Ui,
    Render,
    View,
}

#[derive(Component)]
struct Slider(SliderKind);
#[derive(Component)]
struct SliderFill(SliderKind);
#[derive(Component)]
struct SliderValue(SliderKind);
#[derive(Component)]
struct ToggleText(Ctl);
#[derive(Component)]
struct OpenMenu;
#[derive(Component)]
struct PanelRoot;
/// Preview swatches: (what, which palette colour).
#[derive(Component, Clone, Copy)]
enum Swatch {
    HostileName,
    FriendlyName,
    Health,
    EnemyHealth,
    Danger,
    Safe,
}

#[derive(Resource, Default)]
struct PanelState {
    open: bool,
}

/// Per-entity authored decor distance, kept so presets scale it instead of compounding.
#[derive(Component)]
struct DecorBase(f32);

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let mobile = app
            .world()
            .get_resource::<TouchMode>()
            .is_some_and(|t| t.mobile);
        let saved = crate::net::load_settings();
        // First run: no saved quality, so the perf module measures a few seconds and picks one.
        let first_run = saved.as_deref().is_none_or(|t| !t.contains("quality="));
        let mut settings = saved
            .map(|t| Settings::decode(&t, mobile))
            .unwrap_or_else(|| Settings::defaults(mobile));
        if let Some(q) = std::env::var("WOW_QUALITY").ok().and_then(|v| {
            Quality::ALL
                .into_iter()
                .find(|q| q.key() == v.trim().to_lowercase())
        }) {
            settings.quality = q;
        }
        if let Some(c) = std::env::var("WOW_COLORBLIND").ok().and_then(|v| {
            Colorblind::ALL
                .into_iter()
                .find(|c| c.key() == v.trim().to_lowercase())
        }) {
            settings.colorblind = c;
        }
        if let Ok(v) = std::env::var("WOW_AUTO_LOOT") {
            settings.auto_loot = v.trim() == "1";
        }
        // QA: perf screenshots with the window on, without touching the stored settings.
        if std::env::var_os("WOW_SHOW_FPS").is_some() {
            settings.perf = PerfMode::Compact;
        }
        if let Some(m) = std::env::var("WOW_PERF").ok().and_then(|v| {
            PerfMode::ALL
                .into_iter()
                .find(|m| m.key() == v.trim().to_lowercase())
        }) {
            settings.perf = m;
        }
        let open = std::env::var_os("WOW_SETTINGS").is_some();
        // A forced quality (QA, bench) is an answer already.
        let detect = first_run
            && std::env::var_os("WOW_QUALITY").is_none()
            && crate::debug::bench_spec().is_none();
        app.insert_resource(crate::hud::perf::AutoQuality::new(detect, mobile))
            .insert_resource(palette(settings.colorblind))
            .insert_resource(settings)
            .insert_resource(PanelState { open })
            .add_systems(Startup, spawn_ui)
            .add_systems(PreUpdate, esc_closes.after(InputSystems))
            .add_systems(
                Update,
                (
                    panel_input,
                    sliders,
                    persist_and_palette,
                    panel_visuals,
                    apply_render,
                    apply_decor_range,
                    calm_fx,
                )
                    .chain(),
            );
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, apply_render_scale);
    }
}

/// Esc closes the panel itself instead of unpausing the game underneath.
fn esc_closes(mut keys: ResMut<ButtonInput<KeyCode>>, mut state: ResMut<PanelState>) {
    if state.open && keys.just_pressed(KeyCode::Escape) {
        keys.clear_just_pressed(KeyCode::Escape);
        state.open = false;
    }
}

fn button(ctl: Ctl, text: &str, size: f32) -> impl Bundle {
    (
        Button,
        ToggleText(ctl),
        Node {
            padding: UiRect::axes(px(10), px(3)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
        children![label(text, size)],
    )
}

fn row_label(text: &str) -> impl Bundle {
    (
        Node {
            width: px(130),
            flex_shrink: 0.0,
            ..default()
        },
        children![(label(text.trim(), 12.0), TextColor(DIM))],
    )
}

fn slider(kind: SliderKind) -> impl Bundle {
    (
        Button,
        crate::hud::scroll::NoScrollDrag,
        Slider(kind),
        RelativeCursorPosition::default(),
        Node {
            width: px(200),
            height: px(14),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(BAR_BG),
        children![(
            SliderFill(kind),
            Node {
                height: percent(100),
                width: percent(50),
                ..default()
            },
            BackgroundColor(ACCENT.with_alpha(0.7)),
        )],
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        OpenMenu,
        Node {
            position_type: PositionType::Absolute,
            // Same row as "Co-op (beta)", to its left; "~/keybindings" sits to its right.
            top: percent(74),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![
            (
                Button,
                ToggleText(Ctl::Open),
                Node {
                    margin: UiRect::right(px(320)),
                    padding: UiRect::axes(px(14), px(5)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![label("~/settings", 16.0)],
            ),
            // Players quote this in bug reports; co-op also needs matching builds.
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(44),
                    ..default()
                },
                label(concat!("The Last Dev v", env!("CARGO_PKG_VERSION")), 11.0),
                TextColor(DIM),
            ),
        ],
    ));
    commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(62),
            FocusPolicy::Block,
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(9),
                    padding: UiRect::all(px(16)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(6)),
                    max_height: percent(94),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                crate::hud::scroll::ScrollList::default(),
                BorderColor::all(PANEL_EDGE),
                // Opaque: the Esc menu sits right behind this panel and must not bleed through.
                BackgroundColor(Color::srgb(0.04, 0.045, 0.07)),
            ))
            .with_children(|p| {
                p.spawn((label("~/settings", 18.0), TextColor(ACCENT)));
                p.spawn((
                    label(
                        "// saved on this device. The AI has already picked your settings; these are just suggestions to it.",
                        11.0,
                    ),
                    TextColor(DIM),
                ));
                let row = || Node {
                    column_gap: px(6),
                    align_items: AlignItems::Center,
                    ..default()
                };
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("quality"));
                    for q in Quality::ALL {
                        r.spawn(button(Ctl::Quality(q), q.name(), 13.0));
                    }
                });
                p.spawn((
                    label(
                        "   low: no shadows, bloom or MSAA, short view distance. ultra: all of it, far.",
                        10.0,
                    ),
                    TextColor(DIM),
                ));
                for (kind, name) in [
                    (SliderKind::Fov, "field of view"),
                    (SliderKind::Ui, "UI scale"),
                    (SliderKind::Render, "render scale"),
                    (SliderKind::View, "view distance"),
                ] {
                    p.spawn(row()).with_children(|r| {
                        r.spawn(row_label(name));
                        let (minus, plus) = match kind {
                            SliderKind::Fov => (Ctl::Fov(-1), Ctl::Fov(1)),
                            SliderKind::Ui => (Ctl::Ui(-1), Ctl::Ui(1)),
                            SliderKind::Render => (Ctl::Render(-1), Ctl::Render(1)),
                            SliderKind::View => (Ctl::View(-1), Ctl::View(1)),
                        };
                        r.spawn(button(minus, "-", 13.0));
                        r.spawn(slider(kind));
                        r.spawn(button(plus, "+", 13.0));
                        r.spawn((
                            SliderValue(kind),
                            label("", 12.0),
                            Node {
                                width: px(60),
                                ..default()
                            },
                        ));
                        if kind == SliderKind::Ui {
                            r.spawn(button(Ctl::UiAuto, "auto", 13.0));
                        }
                    });
                }
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("MSAA 4x"));
                    r.spawn(button(Ctl::Msaa, "", 13.0));
                    r.spawn(row_label("shadows"));
                    r.spawn(button(Ctl::Shadows, "", 13.0));
                });
                p.spawn((
                    label(
                        "   render scale applies in the browser only; picking a preset resets these five",
                        10.0,
                    ),
                    TextColor(DIM),
                ));
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("colourblind"));
                    for c in Colorblind::ALL {
                        let name = match c {
                            Colorblind::Off => "Off",
                            Colorblind::Deuteranopia => "Deuteranopia",
                            Colorblind::Protanopia => "Protanopia",
                            Colorblind::Tritanopia => "Tritanopia",
                        };
                        r.spawn(button(Ctl::Colorblind(c), name, 13.0));
                    }
                });
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("preview"));
                    r.spawn((Swatch::HostileName, label("Hostile", 13.0)));
                    r.spawn((Swatch::FriendlyName, label("Friendly", 13.0)));
                    for s in [Swatch::Health, Swatch::EnemyHealth] {
                        r.spawn((
                            s,
                            Node {
                                width: px(56),
                                height: px(9),
                                border_radius: BorderRadius::all(px(2)),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        ));
                    }
                    r.spawn((
                        Swatch::Danger,
                        Node {
                            width: px(26),
                            height: px(26),
                            border: UiRect::all(px(3)),
                            border_radius: BorderRadius::all(px(13)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        BackgroundColor(Color::WHITE),
                        children![(
                            Swatch::Safe,
                            Node {
                                width: px(8),
                                height: px(8),
                                border_radius: BorderRadius::all(px(4)),
                                ..default()
                            },
                            BackgroundColor(Color::WHITE),
                        )],
                    ));
                    r.spawn((label("telegraph / safe spot", 11.0), TextColor(DIM)));
                });
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("reduce motion"));
                    r.spawn(button(Ctl::Motion, "", 13.0));
                });
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("~/perf window"));
                    for m in PerfMode::ALL {
                        let name = match m {
                            PerfMode::Off => "Off",
                            PerfMode::Compact => "Compact",
                            PerfMode::Full => "Full",
                        };
                        r.spawn(button(Ctl::Perf(m), name, 13.0));
                    }
                });
                p.spawn(row()).with_children(|r| {
                    r.spawn(row_label("auto loot"));
                    r.spawn(button(Ctl::AutoLoot, "", 13.0));
                });
                p.spawn(Node {
                    column_gap: px(10),
                    margin: UiRect::top(px(4)),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(button(Ctl::Reset, "Reset to defaults", 14.0));
                    r.spawn(button(Ctl::Close, "Close", 14.0));
                });
            });
        });
}

fn panel_input(
    paused: Res<Paused>,
    lobby: Res<Lobby>,
    touch: Res<TouchMode>,
    mut state: ResMut<PanelState>,
    mut settings: ResMut<Settings>,
    mut auto: ResMut<crate::hud::perf::AutoQuality>,
    buttons: Query<(&Interaction, &ToggleText), Changed<Interaction>>,
    mut menu: Single<&mut Node, (With<OpenMenu>, Without<PanelRoot>)>,
    mut root: Single<&mut Node, (With<PanelRoot>, Without<OpenMenu>)>,
) {
    // The QA hook opens it without a pause.
    let qa = std::env::var_os("WOW_SETTINGS").is_some();
    if state.open && !paused.0 && !qa {
        state.open = false;
    }
    for (i, t) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        let s = &mut *settings;
        match t.0 {
            Ctl::Open if paused.0 && !lobby.open => state.open = true,
            Ctl::Open => {}
            _ if !state.open => {}
            Ctl::Close => state.open = false,
            Ctl::Reset => *s = Settings::defaults(touch.mobile),
            Ctl::Quality(q) => {
                s.set_quality(q);
                // A manual pick settles the question the auto-detect was about to answer.
                auto.cancel();
            }
            Ctl::Colorblind(c) => s.colorblind = c,
            Ctl::Fov(d) => s.fov_deg = (s.fov_deg + f32::from(d) * 5.0).clamp(FOV_MIN, FOV_MAX),
            Ctl::Ui(d) => {
                let cur = s.ui_scale.unwrap_or(1.0);
                s.ui_scale = Some((cur + f32::from(d) * 0.1).clamp(UI_SCALE_MIN, UI_SCALE_MAX));
            }
            Ctl::UiAuto => s.ui_scale = None,
            Ctl::Render(d) => {
                let v = s.render_scale() + f32::from(d) * 0.05;
                s.render_scale = Some(v.clamp(RENDER_SCALE_MIN, RENDER_SCALE_MAX));
            }
            Ctl::View(d) => {
                let v = s.decor_range() + f32::from(d) * 0.1;
                s.view_dist = Some(v.clamp(VIEW_DIST_MIN, VIEW_DIST_MAX));
            }
            Ctl::Msaa => s.msaa = Some(!s.msaa()),
            Ctl::Shadows => s.shadows = Some(!s.shadows()),
            Ctl::Motion => s.reduce_motion = !s.reduce_motion,
            Ctl::Perf(m) => s.perf = m,
            Ctl::AutoLoot => s.auto_loot = !s.auto_loot,
        }
    }
    let show_menu = if paused.0 && !lobby.open && !state.open {
        Display::Flex
    } else {
        Display::None
    };
    if menu.display != show_menu {
        menu.display = show_menu;
    }
    let show = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != show {
        root.display = show;
    }
}

/// Click or drag along a track to set its value.
fn sliders(
    state: Res<PanelState>,
    mut settings: ResMut<Settings>,
    tracks: Query<(&Interaction, &RelativeCursorPosition, &Slider)>,
) {
    if !state.open {
        return;
    }
    for (i, rel, s) in &tracks {
        let Some(n) = rel.normalized.filter(|_| *i == Interaction::Pressed) else {
            continue;
        };
        let t = (n.x + 0.5).clamp(0.0, 1.0);
        match s.0 {
            SliderKind::Fov => {
                let v = (FOV_MIN + t * (FOV_MAX - FOV_MIN)).round();
                if settings.fov_deg != v {
                    settings.fov_deg = v;
                }
            }
            SliderKind::Ui => {
                let v = ((UI_SCALE_MIN + t * (UI_SCALE_MAX - UI_SCALE_MIN)) * 20.0).round() / 20.0;
                if settings.ui_scale != Some(v) {
                    settings.ui_scale = Some(v);
                }
            }
            SliderKind::Render => {
                let v = ((RENDER_SCALE_MIN + t * (RENDER_SCALE_MAX - RENDER_SCALE_MIN)) * 20.0)
                    .round()
                    / 20.0;
                if settings.render_scale != Some(v) {
                    settings.render_scale = Some(v);
                }
            }
            SliderKind::View => {
                let v =
                    ((VIEW_DIST_MIN + t * (VIEW_DIST_MAX - VIEW_DIST_MIN)) * 10.0).round() / 10.0;
                if settings.view_dist != Some(v) {
                    settings.view_dist = Some(v);
                }
            }
        }
    }
}

fn persist_and_palette(settings: Res<Settings>, mut pal: ResMut<Palette>) {
    if !settings.is_changed() || settings.is_added() {
        return;
    }
    crate::net::save_settings(&settings.encode());
    let p = palette(settings.colorblind);
    if *pal != p {
        *pal = p;
    }
}

fn panel_visuals(
    state: Res<PanelState>,
    settings: Res<Settings>,
    pal: Res<Palette>,
    mut buttons: Query<(&ToggleText, &mut BorderColor, &Children)>,
    mut texts: Query<&mut Text>,
    mut fills: Query<(&SliderFill, &mut Node)>,
    values: Query<(&SliderValue, Entity)>,
    mut swatches: Query<
        (
            &Swatch,
            Option<&mut TextColor>,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
        ),
        Without<ToggleText>,
    >,
) {
    if !state.open {
        return;
    }
    let s = &*settings;
    for (t, mut border, kids) in &mut buttons {
        let on = match t.0 {
            Ctl::Quality(q) => Some(s.quality == q),
            Ctl::Msaa => Some(s.msaa()),
            Ctl::Shadows => Some(s.shadows()),
            Ctl::Perf(m) => Some(s.perf == m),
            Ctl::Colorblind(c) => Some(s.colorblind == c),
            Ctl::UiAuto => Some(s.ui_scale.is_none()),
            Ctl::Motion => Some(s.reduce_motion),
            Ctl::AutoLoot => Some(s.auto_loot),
            _ => None,
        };
        let c = if on == Some(true) {
            ACCENT_WARM
        } else {
            KEY_RIM
        };
        border.set_all(c);
        if matches!(t.0, Ctl::Motion | Ctl::Msaa | Ctl::Shadows | Ctl::AutoLoot)
            && let Some(k) = kids.first()
            && let Ok(mut text) = texts.get_mut(*k)
        {
            set_text(&mut text, if on == Some(true) { "on" } else { "off" });
        }
    }
    let fov_t = (s.fov_deg - FOV_MIN) / (FOV_MAX - FOV_MIN);
    let ui = s.ui_scale.unwrap_or(1.0);
    let ui_t = (ui - UI_SCALE_MIN) / (UI_SCALE_MAX - UI_SCALE_MIN);
    let render_t = (s.render_scale() - RENDER_SCALE_MIN) / (RENDER_SCALE_MAX - RENDER_SCALE_MIN);
    let view_t = (s.decor_range() - VIEW_DIST_MIN) / (VIEW_DIST_MAX - VIEW_DIST_MIN);
    for (f, mut node) in &mut fills {
        let t = match f.0 {
            SliderKind::Fov => fov_t,
            SliderKind::Ui => ui_t,
            SliderKind::Render => render_t,
            SliderKind::View => view_t,
        };
        node.width = percent(t.clamp(0.0, 1.0) * 100.0);
    }
    for (v, e) in &values {
        let text = match v.0 {
            SliderKind::Fov => format!("{:.0} deg", s.fov_deg),
            SliderKind::Ui => s
                .ui_scale
                .map_or("auto".into(), |u| format!("{:.0}%", u * 100.0)),
            SliderKind::Render => format!("{:.0}%", s.render_scale() * 100.0),
            SliderKind::View => format!("{:.0}%", s.decor_range() * 100.0),
        };
        if let Ok(mut t) = texts.get_mut(e) {
            set_text(&mut t, &text);
        }
    }
    let rgb = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    for (sw, tc, bg, border) in &mut swatches {
        let c = match sw {
            Swatch::HostileName => pal.hostile,
            Swatch::FriendlyName => pal.friendly,
            Swatch::Health => pal.health,
            Swatch::EnemyHealth => pal.enemy_health,
            Swatch::Danger => rgb(pal.danger).with_alpha(0.45),
            Swatch::Safe => rgb(pal.safe),
        };
        if let Some(mut b) = border {
            b.set_all(c.with_alpha(1.0));
        }
        if matches!(sw, Swatch::HostileName | Swatch::FriendlyName) {
            if let Some(mut tc) = tc {
                tc.0 = c;
            }
        } else if let Some(mut bg) = bg {
            bg.0 = c;
        }
    }
}

/// Shadows, shadow resolution and distance, bloom, MSAA and field of view.
fn apply_render(
    mut commands: Commands,
    settings: Res<Settings>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut lights: Query<(&mut DirectionalLight, Option<&mut CascadeShadowConfig>)>,
    mut cams: Query<(Entity, Has<Bloom>, Option<&Msaa>, &mut Projection), With<OrbitCamera>>,
    new_bloom: Query<(), Added<Bloom>>,
) {
    let q = settings.quality;
    let shadows = settings.shadows();
    // One sun, re-spawned with each floor: checked every frame, written only on a difference.
    for (mut l, cascade) in &mut lights {
        if l.shadow_maps_enabled != shadows {
            l.shadow_maps_enabled = shadows;
        }
        if let Some(mut c) = cascade
            && c.bounds.first().is_some_and(|b| *b != q.shadow_range())
        {
            c.bounds = vec![q.shadow_range()];
        }
    }
    if !settings.is_changed() && new_bloom.is_empty() {
        return;
    }
    if shadow_map.size != q.shadow_map() {
        shadow_map.size = q.shadow_map();
    }
    let msaa = if settings.msaa() {
        Msaa::Sample4
    } else {
        Msaa::Off
    };
    for (e, has_bloom, cur_msaa, mut proj) in &mut cams {
        // arena.rs grades every new camera with bloom; Low takes it away again.
        if has_bloom && !q.bloom() {
            commands.entity(e).remove::<Bloom>();
        } else if !has_bloom && q.bloom() {
            commands.entity(e).insert(Bloom {
                intensity: 0.12,
                ..Bloom::NATURAL
            });
        }
        if cur_msaa.copied().unwrap_or_default() != msaa {
            commands.entity(e).try_insert(msaa);
        }
        if let Projection::Perspective(p) = &mut *proj {
            let fov = settings.fov_deg.to_radians();
            if (p.fov - fov).abs() > 1e-4 {
                p.fov = fov;
            }
        }
    }
}

/// Browser only: fewer canvas pixels for the same page size. A native window is tied to its
/// display's pixels, so there the render scale setting has no effect.
#[cfg(target_arch = "wasm32")]
fn apply_render_scale(
    settings: Res<Settings>,
    touch: Res<TouchMode>,
    mut window: Single<&mut Window>,
) {
    if !settings.is_changed() && !touch.is_changed() {
        return;
    }
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio() as f32);
    // Same ceilings as `touch::initial_resolution`: phones 1.5x, desktops 2x the CSS size.
    let base = dpr.clamp(1.0, if touch.mobile { 1.5 } else { 2.0 });
    let want = (base * settings.render_scale()).max(0.5);
    if (window.resolution.scale_factor() - want).abs() > 0.01 {
        window.resolution.set_scale_factor_override(Some(want));
    }
}

fn apply_decor_range(
    mut commands: Commands,
    settings: Res<Settings>,
    mut ranges: Query<(Entity, &mut VisibilityRange, Option<&DecorBase>)>,
) {
    let k = settings.decor_range();
    let changed = settings.is_changed();
    for (e, mut r, base) in &mut ranges {
        let base = match base {
            Some(b) if changed => b.0,
            Some(_) => continue,
            None => {
                let b = r.end_margin.end;
                commands.entity(e).insert(DecorBase(b));
                b
            }
        };
        // Overview shots use an unbounded range; scaling it would gain nothing.
        if !base.is_finite() || base >= f32::MAX / 2.0 {
            continue;
        }
        let end = (base * k).max(r.start_margin.end);
        if (r.end_margin.end - end).abs() > 0.01 {
            r.end_margin = end..end;
        }
    }
}

fn calm_fx(settings: Res<Settings>, mut budget: ResMut<crate::spell_fx::FxBudget>) {
    if budget.calm != settings.reduce_motion {
        budget.calm = settings.reduce_motion;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let s = Settings {
            quality: Quality::Medium,
            fov_deg: 70.0,
            ui_scale: Some(1.25),
            colorblind: Colorblind::Tritanopia,
            reduce_motion: true,
            perf: PerfMode::Full,
            render_scale: Some(0.75),
            msaa: Some(false),
            shadows: Some(true),
            view_dist: Some(1.2),
            auto_loot: true,
        };
        assert_eq!(Settings::decode(&s.encode(), false), s);
        let auto = Settings::defaults(false);
        assert_eq!(Settings::decode(&auto.encode(), true), auto);
    }

    #[test]
    fn missing_or_bad_values_fall_back_to_the_platform_default() {
        assert_eq!(Settings::decode("", true).quality, Quality::Low);
        assert_eq!(Settings::decode("", false).quality, Quality::High);
        let s = Settings::decode(
            "quality=extreme\nfov=999\nui_scale=NaN\ncolorblind=x\nreduce_motion=yes\njunk\n",
            false,
        );
        assert_eq!(s.quality, Quality::High);
        assert_eq!(s.fov_deg, FOV_MAX);
        assert_eq!(s.ui_scale, None);
        assert_eq!(s.colorblind, Colorblind::Off);
        assert!(!s.reduce_motion);
        assert!(!s.auto_loot);
        assert_eq!(
            Settings::decode("ui_scale=9", false).ui_scale,
            Some(UI_SCALE_MAX)
        );
    }

    #[test]
    fn auto_loot_defaults_on_for_touch_and_off_for_desktop_and_persists() {
        assert!(Settings::decode("", true).auto_loot);
        assert!(!Settings::decode("", false).auto_loot);
        let mut s = Settings::defaults(false);
        s.auto_loot = true;
        assert!(Settings::decode(&s.encode(), false).auto_loot);
        s.auto_loot = false;
        assert!(!Settings::decode(&s.encode(), true).auto_loot);
    }

    #[test]
    fn presets_get_cheaper_downwards() {
        let [low, mid, high, ultra] = Quality::ALL;
        assert!(!low.shadows() && !low.bloom() && !low.msaa());
        assert!(mid.shadows() && high.shadows() && high.msaa() && ultra.msaa());
        assert!(low.decor_range() < high.decor_range());
        assert!(mid.shadow_map() < high.shadow_map());
        for w in Quality::ALL.windows(2) {
            let [a, b] = [w[0], w[1]];
            assert!(a.render_scale() <= b.render_scale());
            assert!(a.npc_range() < b.npc_range());
            assert!(a.max_lights() < b.max_lights());
            assert!(a.shadow_map() <= b.shadow_map());
            assert_eq!(b.lower(), Some(a));
        }
        assert_eq!(low.lower(), None);
        for q in Quality::ALL {
            assert!(q.shadow_map().is_power_of_two());
        }
    }

    #[test]
    fn overrides_win_until_a_preset_is_picked() {
        let mut s = Settings::defaults(false);
        s.msaa = Some(false);
        s.view_dist = Some(0.5);
        assert!(!s.msaa());
        assert!((s.npc_range() - s.quality.npc_range() * 0.5).abs() < 1e-3);
        s.set_quality(Quality::Ultra);
        assert!(s.msaa());
        assert_eq!(s.render_scale(), 1.0);
        let old = Settings::decode("show_fps=1\nrender_scale=0.1\n", false);
        assert_eq!(old.perf, PerfMode::Compact);
        assert_eq!(old.render_scale, Some(RENDER_SCALE_MIN));
    }

    #[test]
    fn colourblind_palettes_split_hostile_from_friendly() {
        // Every palette keeps hostile and friendly apart in brightness or hue, never both near.
        for c in Colorblind::ALL {
            let p = palette(c);
            let (a, b) = (p.hostile.to_srgba(), p.friendly.to_srgba());
            let diff = (a.red - b.red).abs() + (a.green - b.green).abs() + (a.blue - b.blue).abs();
            assert!(diff > 0.6, "{c:?}");
        }
        // Red-green modes carry no red/green pair at all.
        for c in [Colorblind::Deuteranopia, Colorblind::Protanopia] {
            let f = palette(c).friendly.to_srgba();
            assert!(f.blue > f.green && f.blue > f.red, "{c:?}");
        }
    }
}
