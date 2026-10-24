//! `~/radar`: the corner minimap. North up, the player in the middle. The zone outline is the
//! big map's raster, drawn once per zone and slid under a clipped window, so a frame only moves
//! a handful of UI nodes (a pool of dots) and never renders to a texture.

use super::map::{CANVAS_H, CANVAS_W, rasterise};
use super::{ACCENT, ACCENT_WARM, DIM, PANEL_EDGE, giver_mark, label, put, set_text};
use crate::combat::{SimReady, SimView};
use crate::floors::build_zone;
use crate::game::{GameFlow, zone_label};
use crate::input::QuestWindows;
use crate::keybinds::{Action, Keybinds};
use crate::nameplates::PARTY_BLUE;
use crate::player::{Player, PlayerBody};
use bevy::prelude::*;

/// Side of the square view, in HUD px.
const MM: f32 = 150.0;
const PAD: f32 = 6.0;
/// Top margin shared with the unit frames.
pub const TOP: f32 = 26.0;
pub const WIDTH: f32 = MM + 2.0 * PAD + 2.0;
/// Whole widget height (header, view, footer), so the rows below it can start under it.
pub const HEIGHT: f32 = 214.0;
/// Metres from the centre to the view edge at each zoom step.
const RADII: [f32; 5] = [18.0, 28.0, 40.0, 60.0, 90.0];
const DEFAULT_ZOOM: usize = 2;
/// Hostiles farther than this are not drawn, however far the view reaches.
const HOSTILE_RANGE: f32 = 40.0;
const POOL: usize = 64;
const HOSTILE: Color = Color::srgb(1.0, 0.28, 0.25);
const QUEST_TARGET: Color = Color::srgb(1.0, 0.9, 0.2);
const LOOT: Color = Color::srgb(1.0, 0.82, 0.25);
const DOOR_OPEN: Color = Color::srgb(0.4, 1.0, 0.5);
const DOOR_LOCKED: Color = Color::srgb(1.0, 0.42, 0.36);
/// The board's stand spot; the map uses it too.
pub const CALENDAR_ANCHOR: &str = "prop:company_calendar";
const CAL_PX: f32 = 15.0;

/// Minimap marks, rescanned every `MARK_PERIOD` seconds instead of every frame.
#[derive(Default)]
struct MarkCache {
    at: f32,
    zoom: usize,
    marks: Vec<(Vec2, Kind)>,
    /// Where Ill Marie stands, while she is in view of the last scan.
    marie: Option<Vec2>,
    /// The last mark is the one-frame `ILMARI` label and goes again before the next frame.
    flick: bool,
}

/// The single frame in which Ill Marie's label on the campus map reads `ILMARI`
/// (docs/ILMARI_09.md A.4): rolled once when the campus is first up, between 3 and 20 minutes in,
/// at most once per session, only while she is in view of the map. A client timer, never sim.
#[derive(Default)]
struct MarieFlicker {
    due: Option<f32>,
    done: bool,
}

impl MarieFlicker {
    /// Seconds from the roll (`unit` in 0..1) to the flicker.
    fn delay_secs(unit: f32) -> f32 {
        180.0 + unit.clamp(0.0, 1.0) * 1020.0
    }
}
const MARK_PERIOD: f32 = 0.1;

#[derive(Resource)]
struct Radar {
    zoom: usize,
    zone: String,
    lo: Vec2,
    /// Canvas px per metre of the cached raster.
    scale: f32,
}

#[derive(Component)]
struct RadarOpen;
#[derive(Component)]
struct RadarImage;
#[derive(Component)]
struct RadarDot(usize);
#[derive(Component)]
struct RadarGlyph(usize);
#[derive(Component)]
struct RadarArrow;
/// The Company Calendar's icon on the campus, with a badge while there is news.
#[derive(Component)]
struct RadarCalendar;
#[derive(Component)]
struct RadarCalendarBang;
#[derive(Component)]
struct RadarZone;
#[derive(Component)]
struct RadarCoords;
#[derive(Component)]
struct ZoomButton(i32);

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Radar {
            zoom: DEFAULT_ZOOM,
            zone: String::new(),
            lo: Vec2::ZERO,
            scale: 1.0,
        })
        .add_systems(Startup, spawn_radar)
        .add_systems(
            Update,
            (update_radar, calendar_pin).chain().in_set(SimReady),
        );
    }
}

fn zoom_button(step: i32, text: &str) -> impl Bundle {
    (
        Button,
        ZoomButton(step),
        Node {
            width: px(16),
            height: px(16),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BorderColor::all(PANEL_EDGE),
        BackgroundColor(Color::srgba(0.12, 0.14, 0.19, 0.9)),
        children![label(text, 12.0)],
    )
}

fn spawn_radar(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut icons: ResMut<super::IconCache>,
) {
    let calendar = icons.get(&assets, "calendar");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(TOP),
                width: px(WIDTH),
                height: px(HEIGHT),
                padding: UiRect::all(px(PAD)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE.with_alpha(0.7)),
            BackgroundColor(Color::srgba(0.03, 0.035, 0.05, 0.82)),
            GlobalZIndex(19),
        ))
        .with_children(|root| {
            root.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|head| {
                head.spawn((label("~/radar", 12.0), TextColor(DIM)));
                head.spawn(Node {
                    column_gap: px(3),
                    ..default()
                })
                .with_children(|z| {
                    z.spawn(zoom_button(-1, "+"));
                    z.spawn(zoom_button(1, "-"));
                });
            });
            root.spawn((
                Button,
                RadarOpen,
                crate::onboarding::Spot::Minimap,
                Node {
                    width: px(MM),
                    height: px(MM),
                    border: UiRect::all(px(1)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(Color::srgba(0.02, 0.025, 0.04, 0.9)),
            ))
            .with_children(|view| {
                view.spawn((
                    RadarImage,
                    ImageNode::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ));
                for i in 0..POOL {
                    view.spawn((
                        RadarDot(i),
                        Node {
                            position_type: PositionType::Absolute,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            display: Display::None,
                            ..default()
                        },
                        UiTransform::default(),
                        children![(RadarGlyph(i), label("", 11.0), TextColor(Color::NONE))],
                    ));
                }
                view.spawn((
                    RadarCalendar,
                    ImageNode::new(calendar),
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(CAL_PX),
                        height: px(CAL_PX),
                        border: UiRect::all(px(1)),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::WHITE),
                    // A badge on the icon's corner, not a "!" glyph: that one is the quest giver's
                    // and a lone "!" beside the board read as a giver with nobody there.
                    children![(
                        RadarCalendarBang,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(CAL_PX - 5.0),
                            top: px(-4),
                            width: px(7),
                            height: px(7),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(super::GOLD),
                    )],
                ));
                view.spawn((
                    RadarArrow,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(MM / 2.0 - 7.0),
                        top: px(MM / 2.0 - 12.0),
                        width: px(14),
                        height: px(24),
                        ..default()
                    },
                    UiTransform::default(),
                ))
                .with_children(|a| {
                    a.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(5.5),
                            top: px(0),
                            width: px(3),
                            height: px(13),
                            ..default()
                        },
                        BackgroundColor(Color::WHITE),
                    ));
                    a.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(2.5),
                            top: px(7.5),
                            width: px(9),
                            height: px(9),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        BackgroundColor(ACCENT),
                    ));
                });
                view.spawn((
                    label("N", 10.0),
                    TextColor(ACCENT_WARM),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(MM / 2.0 - 3.0),
                        top: px(1),
                        ..default()
                    },
                ));
            });
            root.spawn((RadarZone, label("", 11.0), TextColor(ACCENT)));
            root.spawn((RadarCoords, label("", 10.0), TextColor(DIM)));
        });
}

#[derive(Clone, Copy)]
enum Kind {
    /// `quest`: a tracked quest still needs it (drawn yellow, at any distance in view).
    Hostile {
        boss: bool,
        quest: bool,
    },
    Party,
    Loot,
    Glyph(&'static str, Color),
}

/// Places the calendar icon where the board stands (campus only) and shows its news badge.
fn calendar_pin(
    flow: Res<GameFlow>,
    radar: Res<Radar>,
    cal: Res<crate::calendar::CalendarState>,
    player: Single<&PlayerBody, With<Player>>,
    mut pin: Single<&mut Node, With<RadarCalendar>>,
    mut bang: Single<&mut Visibility, With<RadarCalendarBang>>,
) {
    let me = Vec2::new(player.pos.x, player.pos.z);
    let half = RADII[radar.zoom];
    let at = flow
        .anchors
        .iter()
        .find(|a| a.0 == CALENDAR_ANCHOR)
        .map(|a| Vec2::new(a.1.x, a.1.z))
        .filter(|p| (*p - me).abs().max_element() <= half);
    let Some(p) = at else {
        if pin.display != Display::None {
            pin.display = Display::None;
        }
        return;
    };
    let v = to_view(p, me, (MM / 2.0) / half);
    put!(pin.display, Display::Flex);
    put!(pin.left, px(MM / 2.0 + v.x - CAL_PX / 2.0));
    put!(pin.top, px(MM / 2.0 + v.y - CAL_PX / 2.0));
    bang.set_if_neq(if cal.news {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

/// Zoom index after one step, clamped to the table.
fn step_zoom(zoom: usize, step: i32) -> usize {
    zoom.saturating_add_signed(step as isize)
        .min(RADII.len() - 1)
}

/// Offset in view px of a world point relative to the player (north up: +x right, +z down).
fn to_view(world: Vec2, player: Vec2, px_per_m: f32) -> Vec2 {
    (world - player) * px_per_m
}

fn update_radar(
    (keys, binds, paused, live, time): (
        Res<ButtonInput<KeyCode>>,
        Res<Keybinds>,
        Res<crate::input::Paused>,
        Res<crate::live::LiveState>,
        Res<Time>,
    ),
    sim: SimView,
    flow: Res<GameFlow>,
    mut radar: ResMut<Radar>,
    mut windows: ResMut<QuestWindows>,
    track: Res<super::quests::QuestTrack>,
    mut images: ResMut<Assets<Image>>,
    player: Single<&PlayerBody, With<Player>>,
    (open, zooms): (
        Query<&Interaction, (Changed<Interaction>, With<RadarOpen>)>,
        Query<(&Interaction, &ZoomButton), Changed<Interaction>>,
    ),
    mut image: Single<(&mut ImageNode, &mut Node), (With<RadarImage>, Without<RadarDot>)>,
    mut arrow: Single<&mut UiTransform, (With<RadarArrow>, Without<RadarDot>)>,
    mut dots: Query<
        (
            &RadarDot,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut UiTransform,
        ),
        Without<RadarImage>,
    >,
    mut glyphs: Query<(&RadarGlyph, &mut Text, &mut TextColor)>,
    mut cache: Local<MarkCache>,
    mut flicker: Local<MarieFlicker>,
    (mut zone_text, mut coords): (
        Single<&mut Text, (With<RadarZone>, Without<RadarGlyph>, Without<RadarCoords>)>,
        Single<
            (&mut Text, &mut TextColor),
            (With<RadarCoords>, Without<RadarGlyph>, Without<RadarZone>),
        >,
    ),
) {
    let mut step = 0;
    if !paused.0 {
        if binds.just(&keys, Action::MinimapZoomIn) {
            step -= 1;
        }
        if binds.just(&keys, Action::MinimapZoomOut) {
            step += 1;
        }
    }
    for (i, z) in &zooms {
        if *i == Interaction::Pressed {
            step += z.0;
        }
    }
    radar.zoom = step_zoom(radar.zoom, step);
    if open.iter().any(|i| *i == Interaction::Pressed) {
        windows.map = true;
    }

    if radar.zone != flow.zone
        && let Some(layout) = build_zone(&flow.zone)
    {
        let (img, lo, scale) = rasterise(&layout);
        image.0.image = images.add(img);
        radar.zone.clone_from(&flow.zone);
        radar.lo = lo;
        radar.scale = scale;
        set_text(&mut zone_text, &zone_label(sim.content(), &flow.zone));
    }
    // On the campus the header names the district under the player, like a WoW subzone.
    if flow.zone == "campus" {
        let name = crate::floors::district_at(player.pos.x, player.pos.z)
            .map_or_else(|| zone_label(sim.content(), &flow.zone), str::to_string);
        set_text(&mut zone_text, &name);
    }

    let me = Vec2::new(player.pos.x, player.pos.z);
    let px_per_m = (MM / 2.0) / RADII[radar.zoom];
    // The raster is CANVAS_W x CANVAS_H canvas px at `scale` px per metre.
    let k = px_per_m / radar.scale;
    let at = to_view(radar.lo, me, px_per_m);
    put!(image.1.width, px(CANVAS_W * k));
    put!(image.1.height, px(CANVAS_H * k));
    put!(image.1.left, px(MM / 2.0 + at.x));
    put!(image.1.top, px(MM / 2.0 + at.y));

    // Facing is (-sin yaw, -cos yaw) in (x, z); the arrow art points at -z.
    put!(arrow.rotation, Rot2::radians(-player.yaw));
    // A Legacy floor keeps its affix in view where the coordinates usually are.
    let (coords_text, coords_color) = &mut *coords;
    let legacy = sim.legacy_view();
    match crate::legacy::affix_name(sim.content(), &flow.zone).filter(|_| legacy.zone == flow.zone)
    {
        Some(affix) => {
            set_text(coords_text, &format!("LEGACY  {affix}"));
            put!(coords_color.0, crate::legacy::LEGACY);
        }
        None => {
            set_text(
                coords_text,
                &format!("x {:>4.0}  z {:>4.0}", player.pos.x, player.pos.z),
            );
            put!(coords_color.0, DIM);
        }
    }

    let half = RADII[radar.zoom];
    let in_view = |p: Vec2| (p - me).abs().max_element() <= half;
    // The unit scan (quest lookups per giver) is the costly part and dots are 5-12 px, so it
    // runs at 10 Hz; dot positions below still follow the player every frame.
    let now = time.elapsed_secs();
    if cache.flick {
        cache.marks.pop();
        cache.flick = false;
    }
    if now - cache.at >= MARK_PERIOD || now < cache.at || cache.zoom != radar.zoom {
        let mut marks = std::mem::take(&mut cache.marks);
        marks.clear();
        cache.marie = None;
        let wanted = super::quests::objective_units(&*sim, &track);
        for u in sim.units() {
            let p = Vec2::new(u.pos.x, u.pos.z);
            if Some(u.id) == sim.player() || !in_view(p) {
                continue;
            }
            if u.alive && !u.hostile && u.key.starts_with("ill_marie") {
                cache.marie = Some(p);
            }
            if u.lootable {
                marks.push((p, Kind::Loot));
            } else if !u.alive {
                continue;
            } else if sim.players().contains(&u.id) {
                marks.push((p, Kind::Party));
            } else if u.hostile {
                let quest = wanted.contains(&u.key);
                if quest || p.distance(me) <= HOSTILE_RANGE {
                    marks.push((
                        p,
                        Kind::Hostile {
                            boss: u.boss,
                            quest,
                        },
                    ));
                }
            } else if let Some((g, c)) = giver_mark(&*sim, &u.key, &flow.zone) {
                marks.push((p, Kind::Glyph(g, c)));
            }
        }
        for l in sim.zone_links(&flow.zone) {
            let Some((_, pos, _)) = flow.anchors.iter().find(|a| a.0 == l.anchor) else {
                continue;
            };
            let p = Vec2::new(pos.x, pos.z);
            if in_view(p) {
                let open = l.unlocked || flow.unlock_all;
                marks.push((
                    p,
                    Kind::Glyph("#", if open { DOOR_OPEN } else { DOOR_LOCKED }),
                ));
            }
        }
        // The Board Meeting's spot while its countdown runs (the boss itself is a boss dot later).
        if live.boss_in(time.elapsed_secs()).is_some_and(|s| s > 0)
            && let Some((_, pos, _)) = flow.anchors.iter().find(|a| a.0 == "world_boss")
        {
            let p = Vec2::new(pos.x, pos.z);
            if in_view(p) {
                marks.push((p, Kind::Glyph("B", ACCENT_WARM)));
            }
        }
        // Draw order is pool order: hostiles at the bottom, the group on top.
        let rank = |k: &Kind| match k {
            Kind::Hostile { quest: false, .. } => 0,
            Kind::Hostile { quest: true, .. } => 1,
            Kind::Loot => 2,
            Kind::Glyph(..) => 3,
            Kind::Party => 4,
        };
        marks.sort_by_key(|(_, k)| rank(k));
        marks.truncate(POOL);
        cache.marks = marks;
        cache.at = now;
        cache.zoom = radar.zoom;
    }
    if flow.zone == "campus" && !flicker.done {
        let due = *flicker
            .due
            .get_or_insert_with(|| now + MarieFlicker::delay_secs((now * 7919.0).fract()));
        if now >= due
            && let Some(p) = cache.marie
            && cache.marks.len() < POOL
        {
            cache.marks.push((p, Kind::Glyph("ILMARI", ACCENT_WARM)));
            cache.flick = true;
            flicker.done = true;
        }
    }
    let marks = &cache.marks;

    let mut shown: [Option<(&'static str, Color)>; POOL] = [None; POOL];
    for (dot, mut node, mut bg, mut border, mut tf) in &mut dots {
        let Some((p, kind)) = marks.get(dot.0) else {
            put!(node.display, Display::None);
            continue;
        };
        let (size, fill, edge, turn, glyph) = match kind {
            Kind::Hostile { boss, quest } => {
                let s = if *boss {
                    9.0
                } else if *quest {
                    7.0
                } else {
                    5.0
                };
                let fill = if *quest { QUEST_TARGET } else { HOSTILE };
                (s, fill, Color::srgba(0.0, 0.0, 0.0, 0.8), 0.0, None)
            }
            Kind::Party => (7.0, PARTY_BLUE, Color::WHITE, 0.0, None),
            Kind::Loot => (6.0, LOOT, Color::srgba(0.0, 0.0, 0.0, 0.8), 45.0, None),
            Kind::Glyph(g, c) => (12.0, Color::NONE, Color::NONE, 0.0, Some((*g, *c))),
        };
        let v = to_view(*p, me, px_per_m);
        put!(node.display, Display::Flex);
        put!(node.width, px(size));
        put!(node.height, px(size));
        put!(node.left, px(MM / 2.0 + v.x - size / 2.0));
        put!(node.top, px(MM / 2.0 + v.y - size / 2.0));
        put!(
            node.border,
            if glyph.is_some() {
                UiRect::ZERO
            } else {
                UiRect::all(px(1))
            }
        );
        put!(
            node.border_radius,
            if turn == 0.0 && glyph.is_none() {
                BorderRadius::MAX
            } else {
                BorderRadius::ZERO
            }
        );
        put!(bg.0, fill);
        put!(*border, BorderColor::all(edge));
        put!(tf.rotation, Rot2::degrees(turn));
        shown[dot.0] = glyph;
    }
    for (g, mut text, mut color) in &mut glyphs {
        match shown.get(g.0).copied().flatten() {
            Some((s, c)) => {
                set_text(&mut text, s);
                color.set_if_neq(TextColor(c));
            }
            None => set_text(&mut text, ""),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marie_flickers_between_three_and_twenty_minutes_in() {
        assert_eq!(MarieFlicker::delay_secs(0.0), 180.0);
        assert_eq!(MarieFlicker::delay_secs(1.0), 1200.0);
        assert_eq!(
            MarieFlicker::delay_secs(7.0),
            1200.0,
            "a bad roll is clamped"
        );
    }

    #[test]
    fn zoom_steps_clamp_to_the_table() {
        assert_eq!(step_zoom(0, -1), 0);
        assert_eq!(step_zoom(DEFAULT_ZOOM, 1), DEFAULT_ZOOM + 1);
        assert_eq!(step_zoom(RADII.len() - 1, 1), RADII.len() - 1);
    }

    /// A giver marker is drawn on a sim unit, so a giver is only ever shown where its NPC spawns:
    /// every quest giver and turn-in must be a friendly of its zone whose anchor the layout has,
    /// and the calendar's own pin needs its prop anchor on the campus.
    #[test]
    fn every_quest_giver_has_an_npc_anchor_and_the_calendar_has_its_own() {
        let content = combat_core::Content::from_ron(include_str!("../../../../content/game.ron"))
            .unwrap_or_else(|e| panic!("content: {e}"));
        let anchors = |zone: &str| {
            build_zone(zone)
                .unwrap_or_else(|| panic!("no layout for {zone}"))
                .anchors
                .iter()
                .map(|a| a.0.to_string())
                .collect::<Vec<_>>()
        };
        for q in &content.quests {
            for g in [&q.giver, q.ender()] {
                let z = content
                    .zone(&g.zone)
                    .unwrap_or_else(|| panic!("{}: unknown zone {}", q.id, g.zone));
                let f = z
                    .friendly
                    .iter()
                    .find(|f| f.unit == g.npc)
                    .unwrap_or_else(|| panic!("{}: {} does not spawn in {}", q.id, g.npc, g.zone));
                assert!(
                    anchors(&g.zone).contains(&f.anchor),
                    "{}: anchor {} of {} missing in the {} layout",
                    q.id,
                    f.anchor,
                    g.npc,
                    g.zone
                );
            }
        }
        assert!(anchors("campus").iter().any(|a| a == CALENDAR_ANCHOR));
    }

    #[test]
    fn north_is_up_and_east_is_right() {
        let player = Vec2::new(10.0, 10.0);
        // -z is north: it must land above the centre (negative view y).
        assert!(to_view(Vec2::new(10.0, 0.0), player, 2.0).y < 0.0);
        assert!(to_view(Vec2::new(20.0, 10.0), player, 2.0).x > 0.0);
        assert_eq!(to_view(player, player, 2.0), Vec2::ZERO);
    }
}
