//! `~/perf $ top`: the performance window, plus the quality auto-detect and the "drop one
//! preset" watchdog that read the same frame times.
//!
//! The frame ring is filled every frame (one push); everything else, the entity counts and the
//! text, is recomputed at 4 Hz and only while the window is on, so the readout stays cheap.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::{ACCENT, ACCENT_WARM, Banner, DIM, HP_GREEN, PANEL_EDGE, drop_shadow, label, set_text};
use crate::input::{Paused, TextFocus};
use crate::keybinds::{Action, Keybinds};
use crate::net::{NetClient, NetWorld, clock};
use crate::player::Player;
use crate::settings::{PerfMode, Quality, Settings};
use crate::touch::TouchMode;
use crate::units::Unit;
use bevy::ecs::entity::Entities;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

const REFRESH_SECS: f32 = 0.25;
/// Frames kept for the 1% low; the sparkline shows the newest `SPARK_FRAMES`.
const HISTORY: usize = 300;
const SPARK_FRAMES: usize = 120;
const WIDTH: f32 = 392.0;
/// Frame times above this are hitches (loading, tab switch), not the machine's speed.
const HITCH_SECS: f32 = 0.25;

/// Auto-detect: skip the shader-compile and asset-load frames, then sample this long.
const DETECT_WARMUP_SECS: f32 = 5.0;
const DETECT_SECS: f32 = 3.0;
/// Watchdog: this many consecutive seconds under `LOW_FPS` produce the offer.
const LOW_FPS: f32 = 30.0;
const LOW_SECS: u32 = 20;
const OFFER_SECS: f32 = 15.0;
/// After an offer nothing is offered again for this long, so it can never flap.
const OFFER_COOLDOWN_SECS: f32 = 120.0;

/// Smoothed duration of `combat_core`'s `Sim::tick`, microseconds. A plain static: one writer
/// (the combat bridge), one reader (this window).
static SIM_TICK_US: AtomicU32 = AtomicU32::new(0);

/// Runs one offline sim tick and records how long it took.
pub fn time_sim<T>(tick: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = tick();
    let us = start.elapsed().as_micros().min(u128::from(u32::MAX)) as u32;
    let old = SIM_TICK_US.load(Ordering::Relaxed);
    let smoothed = if old == 0 { us } else { (old * 9 + us) / 10 };
    SIM_TICK_US.store(smoothed, Ordering::Relaxed);
    out
}

pub fn fps_joke(fps: f32) -> &'static str {
    if fps > 55.0 {
        "Performance: exceeds expectations (expectations lowered in Q3)"
    } else if fps >= 30.0 {
        "Works on my machine."
    } else if fps >= 15.0 {
        "Have you tried adding more AI?"
    } else {
        "Rendering outsourced to the AI. Please wait while it hallucinates frames."
    }
}

fn fps_color(fps: f32) -> Color {
    if fps > 55.0 {
        HP_GREEN
    } else if fps >= 30.0 {
        Color::srgb(0.95, 0.8, 0.25)
    } else if fps >= 15.0 {
        ACCENT_WARM
    } else {
        Color::srgb(1.0, 0.3, 0.25)
    }
}

/// One block character per two frames, taller is slower (a full block is 33 ms or worse).
fn sparkline(frames: &VecDeque<f32>) -> String {
    const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let skip = frames.len().saturating_sub(SPARK_FRAMES);
    let v: Vec<f32> = frames.iter().skip(skip).copied().collect();
    v.chunks(2)
        .map(|c| {
            let ms = c.iter().sum::<f32>() / c.len() as f32 * 1000.0;
            BLOCKS[((ms / 33.3 * 7.0).round() as usize).min(7)]
        })
        .collect()
}

/// 1% low: the frame rate of the slowest 1% of recent frames.
fn one_percent_low(frames: &VecDeque<f32>) -> f32 {
    let mut v: Vec<f32> = frames.iter().copied().collect();
    v.sort_unstable_by(f32::total_cmp);
    v.get((v.len() * 99 / 100).min(v.len().saturating_sub(1)))
        .map_or(0.0, |dt| 1.0 / dt.max(1e-4))
}

/// Linux reads statm; the browser reports its wasm heap. Elsewhere there is no cheap, safe way.
fn memory_mb() -> Option<(f32, &'static str)> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let mem = wasm_bindgen::memory().unchecked_into::<js_sys::WebAssembly::Memory>();
        let bytes = mem
            .buffer()
            .unchecked_into::<js_sys::ArrayBuffer>()
            .byte_length();
        Some((bytes as f32 / 1_048_576.0, "wasm heap"))
    }
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages: f32 = statm.split_whitespace().nth(1)?.parse().ok()?;
        Some((pages * 4096.0 / 1_048_576.0, "RSS"))
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
    None
}

/// True for CPU renderers (SwiftShader, llvmpipe): no preset above Low will do there.
#[cfg(target_arch = "wasm32")]
fn software_renderer() -> bool {
    let probe = js_sys::Function::new_no_args(
        "var c=document.createElement('canvas').getContext('webgl');\
         if(!c)return '';var e=c.getExtension('WEBGL_debug_renderer_info');\
         return e?String(c.getParameter(e.UNMASKED_RENDERER_WEBGL)):'';",
    );
    probe
        .call0(&wasm_bindgen::JsValue::NULL)
        .ok()
        .and_then(|v| v.as_string())
        .is_some_and(|r| {
            let r = r.to_lowercase();
            ["swiftshader", "llvmpipe", "software", "basic render"]
                .iter()
                .any(|s| r.contains(s))
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn software_renderer() -> bool {
    false
}

/// Preset for a measured median frame rate. A vsync-capped display can only prove "at least
/// 60", so Ultra needs a high-refresh screen; phones stop at Medium.
fn preset_for(fps: f32, mobile: bool, software: bool) -> Quality {
    let q = if software || fps < 40.0 {
        Quality::Low
    } else if fps < 55.0 {
        Quality::Medium
    } else if fps < 110.0 {
        Quality::High
    } else {
        Quality::Ultra
    };
    if mobile { q.min_medium() } else { q }
}

impl Quality {
    fn min_medium(self) -> Quality {
        if self == Quality::Low {
            Quality::Low
        } else {
            Quality::Medium
        }
    }
}

#[derive(Resource)]
struct PerfStats {
    frames: VecDeque<f32>,
    since_refresh: f32,
    /// Counters and clock at the last refresh, for per-second rates.
    last_net: ([u64; 4], f64),
}

impl Default for PerfStats {
    fn default() -> Self {
        Self {
            frames: VecDeque::with_capacity(HISTORY + 1),
            since_refresh: REFRESH_SECS,
            last_net: ([0; 4], 0.0),
        }
    }
}

/// First-run preset detection and the "drop one preset" watchdog.
#[derive(Resource)]
pub struct AutoQuality {
    detect: bool,
    mobile: bool,
    elapsed: f32,
    samples: Vec<f32>,
    /// (target preset, seconds left to accept).
    offer: Option<(Quality, f32)>,
    // Watchdog: frames and time in the current second, and consecutive slow seconds.
    second: (u32, f32),
    slow_secs: u32,
    cooldown: f32,
}

impl AutoQuality {
    pub fn new(detect: bool, mobile: bool) -> Self {
        Self {
            detect,
            mobile,
            elapsed: 0.0,
            samples: Vec::new(),
            // QA: shows the offer without a 20 s slow stretch.
            offer: std::env::var_os("WOW_PERF_OFFER").map(|_| (Quality::Low, 1e6)),
            second: (0, 0.0),
            slow_secs: 0,
            cooldown: 0.0,
        }
    }

    /// The player chose a preset themselves.
    pub fn cancel(&mut self) {
        self.detect = false;
        self.offer = None;
        self.slow_secs = 0;
        self.cooldown = OFFER_COOLDOWN_SECS;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Fps,
    Frame,
    Spark,
    Joke,
    World,
    Sim,
    Mem,
    Render,
    NetHead,
    NetLatency,
    NetJitter,
    NetSnap,
    NetTraffic,
    NetServer,
}

impl Row {
    const ALL: [Row; 14] = [
        Row::Fps,
        Row::Frame,
        Row::Spark,
        Row::Joke,
        Row::World,
        Row::Sim,
        Row::Mem,
        Row::Render,
        Row::NetHead,
        Row::NetLatency,
        Row::NetJitter,
        Row::NetSnap,
        Row::NetTraffic,
        Row::NetServer,
    ];

    fn net(self) -> bool {
        matches!(
            self,
            Row::NetHead
                | Row::NetLatency
                | Row::NetJitter
                | Row::NetSnap
                | Row::NetTraffic
                | Row::NetServer
        )
    }
}

#[derive(Component)]
struct PerfWindow;
#[derive(Component)]
struct PerfTitle;
#[derive(Component)]
struct PerfCompact;
#[derive(Component)]
struct PerfRow(Row);
#[derive(Component)]
struct OfferRoot;
#[derive(Component)]
struct OfferText;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PerfStats>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (
                    record_frame,
                    toggle_key,
                    refresh,
                    auto_detect,
                    watchdog,
                    show_offer,
                )
                    .chain(),
            );
    }
}

fn spawn(mut commands: Commands) {
    let bar_bg = Color::srgba(0.14, 0.15, 0.2, 0.96);
    commands
        .spawn((
            PerfWindow,
            GlobalZIndex(32),
            Node {
                position_type: PositionType::Absolute,
                width: px(WIDTH),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            drop_shadow(),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    height: px(20),
                    padding: UiRect::horizontal(px(8)),
                    column_gap: px(5),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(bar_bg),
            ))
            .with_children(|bar| {
                for c in [
                    Color::srgb(1.0, 0.37, 0.34),
                    Color::srgb(1.0, 0.75, 0.2),
                    Color::srgb(0.3, 0.8, 0.35),
                ] {
                    bar.spawn((
                        Node {
                            width: px(8),
                            height: px(8),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(c),
                    ));
                }
                bar.spawn((
                    PerfTitle,
                    Node {
                        margin: UiRect::left(px(6)),
                        ..default()
                    },
                    label("~/perf $ top", 10.0),
                    TextLayout::no_wrap(),
                    TextColor(DIM),
                ));
            });
            root.spawn((
                Node {
                    padding: UiRect::axes(px(8), px(6)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.88)),
            ))
            .with_children(|body| {
                for row in Row::ALL {
                    let (size, color) = match row {
                        Row::Fps => (13.0, Color::WHITE),
                        Row::Spark => (10.0, ACCENT),
                        Row::Joke => (10.0, DIM),
                        Row::NetHead => (10.0, ACCENT),
                        _ => (10.5, Color::srgb(0.85, 0.88, 0.94)),
                    };
                    let mut e = body.spawn((
                        PerfRow(row),
                        Node {
                            margin: if matches!(row, Row::World | Row::NetHead) {
                                UiRect::top(px(5))
                            } else {
                                UiRect::ZERO
                            },
                            ..default()
                        },
                        label("", size),
                        TextColor(color),
                    ));
                    // A wrapped joke is the one row allowed two lines.
                    if row != Row::Joke {
                        e.insert(TextLayout::no_wrap());
                    }
                }
            });
        });
    commands.spawn((
        PerfCompact,
        GlobalZIndex(32),
        Node {
            position_type: PositionType::Absolute,
            padding: UiRect::axes(px(8), px(2)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.78)),
        label("", 11.0),
        TextLayout::no_wrap(),
    ));
    commands.spawn((
        OfferRoot,
        GlobalZIndex(33),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        children![(
            OfferText,
            Button,
            Node {
                padding: UiRect::axes(px(10), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(ACCENT_WARM),
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.92)),
            label("", 12.0),
            TextLayout::no_wrap(),
        )],
    ));
}

fn record_frame(time: Res<Time>, mut stats: ResMut<PerfStats>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // A hitch stays in the ring (it is what a 1% low is about); only the clock is bounded.
    if stats.frames.len() == HISTORY {
        stats.frames.pop_front();
    }
    stats.frames.push_back(dt);
    stats.since_refresh += dt;
}

fn toggle_key(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    mut settings: ResMut<Settings>,
) {
    if !paused.0 && !focus.0 && binds.just(&keys, Action::Perf) {
        settings.perf = settings.perf.next();
    }
}

/// Both sit in the free band under the key-hint row, between the player frames and the radar
/// column. Touch scales the HUD up, so it starts lower, under the Log and Menu buttons.
fn place(node: &mut Node, compact: bool, touch: bool) {
    let (left, top) = if touch {
        (px(280), px(64))
    } else {
        (px(300), px(if compact { 40 } else { 44 }))
    };
    if node.left != left {
        node.left = left;
    }
    if node.top != top {
        node.top = top;
    }
}

fn refresh(
    settings: Res<Settings>,
    touch: Res<TouchMode>,
    binds: Res<Keybinds>,
    mut stats: ResMut<PerfStats>,
    entities: &Entities,
    client: Res<NetClient>,
    net: Option<Res<NetWorld>>,
    player: Query<&Transform, With<Player>>,
    npcs: Query<&Transform, With<Unit>>,
    meshes: Query<&ViewVisibility, With<Mesh3d>>,
    points: Query<&ViewVisibility, With<PointLight>>,
    mut window: Single<&mut Node, (With<PerfWindow>, Without<PerfCompact>)>,
    mut compact: Single<
        (&mut Node, &mut Text, &mut TextColor),
        (With<PerfCompact>, Without<PerfWindow>, Without<PerfRow>),
    >,
    mut title: Single<&mut Text, (With<PerfTitle>, Without<PerfRow>, Without<PerfCompact>)>,
    mut rows: Query<
        (&PerfRow, &mut Text, &mut TextColor, &mut Node),
        (
            Without<PerfWindow>,
            Without<PerfCompact>,
            Without<PerfTitle>,
        ),
    >,
) {
    let mode = settings.perf;
    let show = |on: bool| if on { Display::Flex } else { Display::None };
    if window.display != show(mode == PerfMode::Full) {
        window.display = show(mode == PerfMode::Full);
    }
    let (cnode, ctext, ccolor) = &mut *compact;
    if cnode.display != show(mode == PerfMode::Compact) {
        cnode.display = show(mode == PerfMode::Compact);
    }
    if mode == PerfMode::Off || stats.since_refresh < REFRESH_SECS {
        return;
    }
    stats.since_refresh = 0.0;
    place(&mut window, false, touch.active);
    place(cnode, true, touch.active);

    let n = stats.frames.len().clamp(1, 60);
    let recent: f32 = stats.frames.iter().rev().take(n).sum::<f32>() / n as f32;
    let fps = 1.0 / recent.max(1e-4);
    let ms = recent * 1000.0;
    let color = fps_color(fps);
    let q = settings.quality;
    let latency_ms = net.as_ref().map(|w| w.link.rtt_s * 1000.0);

    if mode == PerfMode::Compact {
        let mut line = format!("FPS {fps:.0}  {ms:.1} ms");
        if let Some(l) = latency_ms {
            line.push_str(&format!("  ping {l:.0} ms"));
        }
        line.push_str(&format!("  // {}", fps_joke(fps)));
        set_text(ctext, &line);
        if ccolor.0 != color {
            ccolor.0 = color;
        }
        return;
    }

    let key = binds.label(Action::Perf);
    set_text(&mut title, &format!("~/perf $ top        [{key}] cycles"));

    let visible = |it: &mut dyn Iterator<Item = &ViewVisibility>| it.filter(|v| v.get()).count();
    let mesh_total = meshes.iter().count();
    let mesh_seen = visible(&mut meshes.iter());
    let light_total = points.iter().count();
    let light_seen = visible(&mut points.iter());
    let near = player.single().map_or(0, |p| {
        let r2 = settings.npc_range().powi(2);
        npcs.iter()
            .filter(|t| t.translation.distance_squared(p.translation) <= r2)
            .count()
    });
    let sim_us = SIM_TICK_US.load(Ordering::Relaxed);
    let (new_traffic, now) = (client.traffic(), clock());
    let per_s = |i: usize| {
        let dt = (now - stats.last_net.1).max(0.05);
        (new_traffic[i].saturating_sub(stats.last_net.0[i])) as f64 / dt
    };
    let rates = [per_s(0), per_s(1), per_s(2), per_s(3)];
    stats.last_net = (new_traffic, now);
    let spark = sparkline(&stats.frames);
    let low = one_percent_low(&stats.frames);
    let scale = if cfg!(target_arch = "wasm32") {
        format!("{:.0}%", settings.render_scale() * 100.0)
    } else {
        "native".into()
    };
    let on = |b: bool| if b { "on" } else { "off" };

    for (PerfRow(row), mut text, mut tcolor, mut node) in &mut rows {
        let mem = memory_mb();
        let hidden = (row.net() && net.is_none()) || (*row == Row::Mem && mem.is_none());
        if node.display != show(!hidden) {
            node.display = show(!hidden);
        }
        if hidden {
            continue;
        }
        let line = match row {
            Row::Fps => {
                if tcolor.0 != color {
                    tcolor.0 = color;
                }
                format!("FPS {fps:.0} (target: 60, per the AI roadmap: 240)")
            }
            Row::Frame => format!("frame {ms:.1} ms   1% low {low:.0} fps"),
            Row::Spark => spark.clone(),
            Row::Joke => format!("// {}", fps_joke(fps)),
            Row::World => format!(
                "entities {}   meshes {mesh_seen}/{mesh_total}   lights {light_seen}/{light_total}   npcs {near}",
                entities.count_spawned()
            ),
            Row::Sim if net.is_some() => "sim tick: runs on the server (see below)".to_owned(),
            Row::Sim => format!("sim tick {sim_us} us"),
            Row::Mem => mem.map_or_else(String::new, |(mb, what)| {
                format!("memory {mb:.0} MB ({what})")
            }),
            Row::Render => format!(
                "preset {}  scale {scale}  MSAA {}  shadows {}",
                q.name(),
                if settings.msaa() { "4x" } else { "off" },
                on(settings.shadows()),
            ),
            Row::NetHead => "-- network ---------------------------".to_owned(),
            Row::NetLatency => net.as_ref().map_or_else(String::new, |w| {
                format!(
                    "Latency: {:.0} ms (the AI says it's 0 ms, it checked itself)",
                    w.link.rtt_s * 1000.0
                )
            }),
            Row::NetJitter => net.as_ref().map_or_else(String::new, |w| {
                format!(
                    "rtt avg {:.0} ms   jitter {:.0} ms",
                    w.rtt_avg_s() * 1000.0,
                    w.link.jitter_s * 1000.0
                )
            }),
            Row::NetSnap => net.as_ref().map_or_else(String::new, |w| {
                let age = w
                    .snapshot_age_s(now)
                    .map_or("-".to_owned(), |a| format!("{:.0} ms", a * 1000.0));
                format!(
                    "snapshot age {age}   interp delay {:.0} ms",
                    w.interp_delay_s() * 1000.0
                )
            }),
            Row::NetTraffic => format!(
                "down {:.1} kB/s ({:.0}/s)   up {:.1} kB/s ({:.0}/s)",
                rates[3] / 1000.0,
                rates[2],
                rates[1] / 1000.0,
                rates[0]
            ),
            Row::NetServer => net.as_ref().map_or_else(String::new, |w| {
                w.link
                    .tick_p99_us
                    .map_or("server tick p99 -".to_owned(), |us| {
                        format!("server tick p99 {us} us")
                    })
            }),
        };
        set_text(&mut text, &line);
    }
}

fn auto_detect(
    time: Res<Time>,
    paused: Res<Paused>,
    mut auto: ResMut<AutoQuality>,
    mut settings: ResMut<Settings>,
    mut banners: MessageWriter<Banner>,
) {
    if !auto.detect || paused.0 {
        return;
    }
    let dt = time.delta_secs();
    auto.elapsed += dt;
    if auto.elapsed < DETECT_WARMUP_SECS {
        return;
    }
    if dt > 0.0 && dt < HITCH_SECS {
        auto.samples.push(dt);
    }
    if auto.elapsed < DETECT_WARMUP_SECS + DETECT_SECS {
        return;
    }
    auto.detect = false;
    let mut s = std::mem::take(&mut auto.samples);
    s.sort_unstable_by(f32::total_cmp);
    let Some(median) = s.get(s.len() / 2) else {
        return;
    };
    let q = preset_for(1.0 / median, auto.mobile, software_renderer());
    settings.set_quality(q);
    // Rewrites the file even when the preset equals the default, so this runs once per device.
    settings.set_changed();
    banners.write(Banner {
        title: format!("Quality auto-set to {}", q.name()),
        subtitle: "(overruled by management)".into(),
        color: ACCENT,
    });
}

fn watchdog(
    time: Res<Time>,
    paused: Res<Paused>,
    focus: Res<TextFocus>,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    tap: Query<&Interaction, (With<OfferText>, Changed<Interaction>)>,
    mut auto: ResMut<AutoQuality>,
    mut settings: ResMut<Settings>,
    mut banners: MessageWriter<Banner>,
) {
    let dt = time.delta_secs();
    // Detection owns the first seconds; a menu or a hidden tab is not the machine being slow.
    if auto.detect || paused.0 || dt <= 0.0 || dt >= HITCH_SECS {
        return;
    }
    auto.cooldown = (auto.cooldown - dt).max(0.0);
    if let Some((target, left)) = auto.offer {
        let left = left - dt;
        let tapped = tap.iter().any(|i| *i == Interaction::Pressed);
        if tapped || (!focus.0 && binds.just(&keys, Action::QualityDown)) {
            settings.set_quality(target);
            auto.offer = None;
            auto.slow_secs = 0;
            auto.cooldown = OFFER_COOLDOWN_SECS;
            banners.write(Banner {
                title: format!("Quality lowered to {}", target.name()),
                subtitle: "(the AI approves the downgrade)".into(),
                color: ACCENT,
            });
        } else if left <= 0.0 {
            auto.offer = None;
        } else {
            auto.offer = Some((target, left));
        }
        return;
    }
    auto.second.0 += 1;
    auto.second.1 += dt;
    if auto.second.1 < 1.0 {
        return;
    }
    let fps = auto.second.0 as f32 / auto.second.1;
    auto.second = (0, 0.0);
    auto.slow_secs = if fps < LOW_FPS { auto.slow_secs + 1 } else { 0 };
    if auto.slow_secs >= LOW_SECS
        && auto.cooldown <= 0.0
        && let Some(lower) = settings.quality.lower()
    {
        auto.offer = Some((lower, OFFER_SECS));
        auto.slow_secs = 0;
        auto.cooldown = OFFER_COOLDOWN_SECS;
    }
}

fn show_offer(
    auto: Res<AutoQuality>,
    binds: Res<Keybinds>,
    touch: Res<TouchMode>,
    mut root: Single<&mut Node, With<OfferRoot>>,
    mut text: Single<&mut Text, With<OfferText>>,
) {
    let Some((target, _)) = auto.offer else {
        if root.display != Display::None {
            root.display = Display::None;
        }
        return;
    };
    if root.display != Display::Flex {
        root.display = Display::Flex;
    }
    // Above the action bar, which is where the eyes are during a fight.
    let bottom = px(if touch.active { 110 } else { 96 });
    if root.bottom != bottom {
        root.bottom = bottom;
    }
    set_text(
        &mut text,
        &format!(
            "Struggling. {} to drop to {} (management calls it a perk).",
            if touch.active {
                "Tap here".to_owned()
            } else {
                format!("Press [{}] or click", binds.label(Action::QualityDown))
            },
            target.name()
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jokes_follow_the_bands() {
        assert!(fps_joke(120.0).starts_with("Performance"));
        assert_eq!(fps_joke(40.0), "Works on my machine.");
        assert!(fps_joke(20.0).contains("more AI"));
        assert!(fps_joke(5.0).contains("hallucinates"));
    }

    #[test]
    fn detection_picks_cheaper_presets_for_slower_machines() {
        assert_eq!(preset_for(30.0, false, false), Quality::Low);
        assert_eq!(preset_for(45.0, false, false), Quality::Medium);
        assert_eq!(preset_for(60.0, false, false), Quality::High);
        assert_eq!(preset_for(144.0, false, false), Quality::Ultra);
        assert_eq!(preset_for(144.0, true, false), Quality::Medium);
        assert_eq!(preset_for(60.0, false, true), Quality::Low);
    }

    #[test]
    fn one_percent_low_sees_the_slow_frames() {
        let mut f: VecDeque<f32> = std::iter::repeat_n(1.0 / 60.0, 99).collect();
        f.push_back(0.1);
        assert!((one_percent_low(&f) - 10.0).abs() < 0.5);
        assert_eq!(sparkline(&f).chars().count(), 50);
    }
}
