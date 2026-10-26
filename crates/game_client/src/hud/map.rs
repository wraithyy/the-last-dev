//! `~/map` (N): a top-down drawing of the current zone (rasterised once per zone from the layout's
//! floor and walls) with the player, quest givers, the boss and the doors on it, next to the zone
//! graph with each zone's state. Built only while open.

use super::{ACCENT, DIM, PANEL_EDGE, drop_shadow, giver_mark, label, panel_bg, put, set_text};
use crate::combat::{SimRead, SimReady, SimView};
use crate::floors::{FloorLayout, build_zone};
use crate::game::{GameFlow, lock_reason, zone_label};
use crate::input::{Paused, QuestWindows};
use crate::player::Player;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub(super) const CANVAS_W: f32 = 440.0;
pub(super) const CANVAS_H: f32 = 330.0;
/// Raster pixels per UI pixel: sharp enough at UI scale 2.
const RES: f32 = 2.0;
const FLOOR_RGBA: [u8; 4] = [38, 44, 58, 235];
const WALL_RGBA: [u8; 4] = [120, 150, 190, 255];
/// Furniture, buildings and racks: everything that blocks movement.
const BLOCK_RGBA: [u8; 4] = [70, 84, 108, 255];
/// Outdoors: roads and car parks, footpaths and plazas, water.
const ROAD_RGBA: [u8; 4] = [92, 96, 104, 255];
const PATH_RGBA: [u8; 4] = [66, 74, 86, 255];
const WATER_RGBA: [u8; 4] = [88, 160, 212, 255];
/// Water pixels next to land: a light shoreline instead of a hard dark edge, so a river reads
/// as water and not as a blue road.
const SHORE_RGBA: [u8; 4] = [160, 214, 236, 255];
const OPEN: Color = Color::srgb(0.4, 1.0, 0.5);
const LOCKED: Color = Color::srgb(1.0, 0.42, 0.36);
const QUEST: Color = Color::srgb(1.0, 0.9, 0.2);
/// District names on the campus map: readable, but quieter than markers.
const DISTRICT: Color = Color::srgba(0.75, 0.85, 1.0, 0.55);

#[derive(Component)]
struct MapRoot;
#[derive(Component)]
struct MapCanvas;
#[derive(Component)]
struct MapTitle;
#[derive(Component)]
struct MapClose;
#[derive(Component)]
struct MapGraph;
#[derive(Component)]
struct PlayerDot;
/// Markers that are rebuilt when their state changes (givers, doors, boss).
#[derive(Component)]
struct MapMarker;

/// World-to-canvas transform of the drawn zone.
#[derive(Resource, Default)]
struct MapView {
    zone: String,
    min: Vec2,
    scale: f32,
    shown: String,
}

impl MapView {
    fn to_canvas(&self, p: Vec3) -> Vec2 {
        (Vec2::new(p.x, p.z) - self.min) * self.scale
    }
}

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapView>()
            .add_systems(Startup, spawn_map)
            .add_systems(Update, update_map.in_set(SimReady));
    }
}

fn spawn_map(mut commands: Commands) {
    commands
        .spawn((
            MapRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(34),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(8),
                    padding: UiRect::all(px(10)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    max_height: percent(94),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                super::scroll::ScrollList::default(),
                BorderColor::all(ACCENT.with_alpha(0.7)),
                panel_bg(),
                drop_shadow(),
            ))
            .with_children(|w| {
                super::window_header(
                    w,
                    "Map",
                    (MapTitle, label("~/map", 11.0), TextColor(DIM)),
                    crate::keybinds::Action::Map,
                    MapClose,
                );
                w.spawn(Node {
                    column_gap: px(12),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        MapCanvas,
                        ImageNode::default(),
                        Node {
                            width: px(CANVAS_W),
                            height: px(CANVAS_H),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(PANEL_EDGE),
                        BackgroundColor(Color::srgba(0.02, 0.025, 0.04, 0.9)),
                    ))
                    .with_child((
                        PlayerDot,
                        Node {
                            position_type: PositionType::Absolute,
                            width: px(9),
                            height: px(9),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(5)),
                            ..default()
                        },
                        BorderColor::all(Color::WHITE),
                        BackgroundColor(ACCENT),
                        GlobalZIndex(36),
                    ));
                    row.spawn((
                        MapGraph,
                        super::scroll::ScrollList::default(),
                        Node {
                            width: px(330),
                            // A long tree scrolls beside the map instead of stretching the window.
                            max_height: px(CANVAS_H + 80.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ));
                });
                w.spawn((
                    label(
                        "you  ! ticket  ? review  B boss  door: green open / red locked  (N or Esc to close)",
                        11.0,
                    ),
                    TextColor(DIM),
                ));
            });
        });
}

/// Floor runs filled and walls stroked into an RGBA raster; returns the image and the view.
pub(super) fn rasterise(layout: &FloorLayout) -> (Image, Vec2, f32) {
    let runs = layout.footprint.floor_runs();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for &(x0, x1, z0, z1) in &runs {
        lo = lo.min(Vec2::new(x0, z0));
        hi = hi.max(Vec2::new(x1, z1));
    }
    if lo.x > hi.x {
        (lo, hi) = (Vec2::splat(-1.0), Vec2::splat(1.0));
    }
    let pad = 3.0;
    let (lo, hi) = (lo - pad, hi + pad);
    let scale = (CANVAS_W / (hi.x - lo.x)).min(CANVAS_H / (hi.y - lo.y));
    let (w, h) = ((CANVAS_W * RES) as usize, (CANVAS_H * RES) as usize);
    let mut data = vec![0u8; w * h * 4];
    let px_scale = scale * RES;
    let mut put = |x: i64, y: i64, c: [u8; 4]| {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let i = (y as usize * w + x as usize) * 4;
            data[i..i + 4].copy_from_slice(&c);
        }
    };
    for &(x0, x1, z0, z1) in &runs {
        let (a, b) = (
            (Vec2::new(x0, z0) - lo) * px_scale,
            (Vec2::new(x1, z1) - lo) * px_scale,
        );
        for y in a.y.floor() as i64..b.y.ceil() as i64 {
            for x in a.x.floor() as i64..b.x.ceil() as i64 {
                put(x, y, FLOOR_RGBA);
            }
        }
    }
    let ground: Vec<_> = ground_marks(layout).collect();
    for (mark, c) in &ground {
        if *c != WATER_RGBA {
            fill_mark(&mut put, mark, lo, px_scale, *c);
        }
    }
    for b in &layout.blockers {
        let (a, c) = (
            (Vec2::new(b.min.x, b.min.z) - lo) * px_scale,
            (Vec2::new(b.max.x, b.max.z) - lo) * px_scale,
        );
        for y in a.y.floor() as i64..c.y.ceil() as i64 {
            for x in a.x.floor() as i64..c.x.ceil() as i64 {
                put(x, y, BLOCK_RGBA);
            }
        }
    }
    // Water goes over the invisible river and lake blockers, so it reads as water.
    for (mark, c) in &ground {
        if *c == WATER_RGBA {
            fill_mark(&mut put, mark, lo, px_scale, *c);
        }
    }
    for s in layout.footprint.walls() {
        let (a, b) = ((s.a - lo) * px_scale, (s.b - lo) * px_scale);
        let steps = a.distance(b).ceil().max(1.0) as i64;
        for k in 0..=steps {
            let p = a.lerp(b, k as f32 / steps as f32);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                put(p.x as i64 + dx, p.y as i64 + dy, WALL_RGBA);
            }
        }
    }
    let water: Vec<bool> = data.chunks_exact(4).map(|p| p == WATER_RGBA).collect();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if water[i] && !(water[i - 1] && water[i + 1] && water[i - w] && water[i + w]) {
                data[i * 4..i * 4 + 4].copy_from_slice(&SHORE_RGBA);
            }
        }
    }
    let img = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    (img, lo, scale)
}

/// A flat piece of ground worth drawing on the map: centre, half extents, yaw, round.
struct Mark {
    center: Vec2,
    half: Vec2,
    yaw: f32,
    round: bool,
}

/// Roads, paths and water of an outdoor layout, from its flat pieces.
fn ground_marks(layout: &FloorLayout) -> impl Iterator<Item = (Mark, [u8; 4])> + '_ {
    use crate::floors::kit_for_map::{Mat, Piece, PrimKind};
    let color = |m: Mat, flat: bool| match m {
        Mat::Asphalt => Some(ROAD_RGBA),
        Mat::Water | Mat::Lake => Some(WATER_RGBA),
        Mat::Sand | Mat::Concrete if flat => Some(PATH_RGBA),
        _ => None,
    };
    layout.pieces.iter().filter_map(move |p| {
        if !layout.outdoor {
            return None;
        }
        let (center, size, yaw, mat, round) = match p {
            Piece::Box {
                center, size, mat, ..
            } => (*center, *size, 0.0, *mat, false),
            Piece::Slab {
                center,
                size,
                yaw,
                mat,
                ..
            } => (*center, *size, *yaw, *mat, false),
            Piece::Prim {
                kind: PrimKind::Cylinder,
                center,
                size,
                mat,
                ..
            } => (*center, *size, 0.0, *mat, true),
            _ => return None,
        };
        // Only ground cover: thin, and big enough to matter at map scale.
        let flat = size.y <= 0.15 && size.x.max(size.z) >= 2.0;
        let c = color(mat, flat).filter(|_| size.y <= 0.15 || mat == Mat::Lake)?;
        Some((
            Mark {
                center: Vec2::new(center.x, center.z),
                half: Vec2::new(size.x, size.z) / 2.0,
                yaw,
                round,
            },
            c,
        ))
    })
}

fn fill_mark(
    put: &mut impl FnMut(i64, i64, [u8; 4]),
    m: &Mark,
    lo: Vec2,
    px_scale: f32,
    c: [u8; 4],
) {
    let r = m.half.length();
    let (a, b) = (
        (m.center - Vec2::splat(r) - lo) * px_scale,
        (m.center + Vec2::splat(r) - lo) * px_scale,
    );
    let (s, co) = m.yaw.sin_cos();
    for y in a.y.floor() as i64..b.y.ceil() as i64 {
        for x in a.x.floor() as i64..b.x.ceil() as i64 {
            let w = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / px_scale + lo - m.center;
            let inside = if m.round {
                (w / m.half.max(Vec2::splat(0.01))).length_squared() <= 1.0
            } else {
                // Slab yaw is `Quat::from_rotation_y`: local x runs along (cos, -sin).
                let l = Vec2::new(w.x * co - w.y * s, w.x * s + w.y * co);
                l.x.abs() <= m.half.x && l.y.abs() <= m.half.y
            };
            if inside {
                put(x, y, c);
            }
        }
    }
}

/// Zone graph from the campus outward: (depth, zone id) in walk order.
fn zone_tree(sim: &dyn SimRead) -> Vec<(usize, String)> {
    fn walk(sim: &dyn SimRead, id: &str, depth: usize, out: &mut Vec<(usize, String)>) {
        out.push((depth, id.to_string()));
        for l in sim.zone_links(id) {
            if !out.iter().any(|(_, z)| *z == l.to) {
                walk(sim, &l.to, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(sim, "campus", 0, &mut out);
    out
}

/// Open when the player can walk there from the campus through open links; else the reason of
/// the first gated link into it from a reachable zone (any gated link if none is reachable).
fn zone_state(sim: &dyn SimRead, id: &str, unlock_all: bool) -> Result<(), String> {
    let mut reach = vec!["campus".to_string()];
    let mut i = 0;
    while i < reach.len() {
        for l in sim.zone_links(&reach[i]) {
            if (l.unlocked || unlock_all) && !reach.contains(&l.to) {
                reach.push(l.to);
            }
        }
        i += 1;
    }
    if reach.iter().any(|z| z == id) {
        return Ok(());
    }
    let content = sim.content();
    let gated = |from_reachable: bool| {
        content
            .zones
            .iter()
            .filter(|z| !from_reachable || reach.contains(&z.id))
            .flat_map(|z| z.links.iter())
            .find(|l| l.to == id && l.requires.is_some())
            .map(|l| lock_reason(content, l.requires.as_ref()))
    };
    // An ungated room behind a locked one: name the room in front of it.
    let behind = || {
        content
            .zones
            .iter()
            .find(|z| z.links.iter().any(|l| l.to == id))
            .map(|z| format!("Requires: reach {}", zone_label(content, &z.id)))
    };
    Err(gated(true)
        .or_else(|| gated(false))
        .or_else(behind)
        .unwrap_or_else(|| "Requires: an earlier zone".into()))
}

fn marker(c: &mut ChildSpawnerCommands, at: Vec2, glyph: &str, size: f32, color: Color) {
    c.spawn((
        MapMarker,
        label(glyph, size),
        TextColor(color),
        Node {
            position_type: PositionType::Absolute,
            left: px(at.x - size * 0.3),
            top: px(at.y - size * 0.6),
            ..default()
        },
        GlobalZIndex(35),
    ));
}

fn update_map(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    sim: SimView,
    flow: Res<GameFlow>,
    (mut windows, track, live, time, cal, assets, mut icons, close): (
        ResMut<QuestWindows>,
        Res<super::quests::QuestTrack>,
        Res<crate::live::LiveState>,
        Res<Time>,
        Res<crate::calendar::CalendarState>,
        Res<AssetServer>,
        ResMut<super::IconCache>,
        Query<&Interaction, (With<MapClose>, Changed<Interaction>)>,
    ),
    mut view: ResMut<MapView>,
    mut images: ResMut<Assets<Image>>,
    mut root: Single<&mut Node, (With<MapRoot>, Without<PlayerDot>)>,
    mut canvas: Single<(Entity, &mut ImageNode), With<MapCanvas>>,
    mut title: Single<&mut Text, With<MapTitle>>,
    graph: Single<Entity, With<MapGraph>>,
    mut dot: Single<&mut Node, (With<PlayerDot>, Without<MapRoot>)>,
    markers: Query<Entity, With<MapMarker>>,
    player: Single<&Transform, With<Player>>,
) {
    if !paused.0 && binds.just(&keys, crate::keybinds::Action::Map) {
        windows.map = !windows.map;
    }
    if close.iter().any(|i| *i == Interaction::Pressed) {
        windows.map = false;
    }
    put!(
        root.display,
        if windows.map {
            Display::Flex
        } else {
            Display::None
        }
    );
    if !windows.map {
        view.shown.clear();
        return;
    }
    let content = sim.content();
    if view.zone != flow.zone {
        let Some(layout) = build_zone(&flow.zone) else {
            return;
        };
        let (img, min, scale) = rasterise(&layout);
        canvas.1.image = images.add(img);
        *view = MapView {
            zone: flow.zone.clone(),
            min,
            scale,
            shown: String::new(),
        };
        set_text(
            &mut title,
            &format!("~/map  {}", zone_label(content, &flow.zone)),
        );
    }
    let p = view.to_canvas(player.translation);
    put!(dot.left, px(p.x - 4.5));
    put!(dot.top, px(p.y - 4.5));

    // Givers, boss, quest targets and doors: rebuilt only when something on them changes.
    let wanted = super::quests::objective_units(&*sim, &track);
    let marks: Vec<(Vec2, &str, Color)> = sim
        .units()
        .filter(|u| u.alive)
        .filter_map(|u| {
            let at = view.to_canvas(Vec3::new(u.pos.x, 0.0, u.pos.z));
            if u.boss {
                let c = if wanted.contains(&u.key) {
                    QUEST
                } else {
                    LOCKED
                };
                Some((at, "B", c))
            } else if !u.hostile {
                giver_mark(&*sim, &u.key, &flow.zone).map(|(g, c)| (at, g, c))
            } else if wanted.contains(&u.key) {
                Some((at, "*", QUEST))
            } else {
                None
            }
        })
        .collect();
    // The Board Meeting's spot while its countdown runs.
    let mut marks = marks;
    if live.boss_in(time.elapsed_secs()).is_some_and(|s| s > 0)
        && let Some((_, pos, _)) = flow.anchors.iter().find(|a| a.0 == "world_boss")
    {
        marks.push((view.to_canvas(*pos), "B", super::ACCENT_WARM));
    }
    // World events warned or running: an "E" at their anchor.
    for e in &live.events {
        let anchor = content
            .live
            .events
            .iter()
            .find(|d| d.key == e.key)
            .map(|d| d.anchor.as_str());
        if let Some((_, pos, _)) = flow.anchors.iter().find(|a| Some(a.0) == anchor) {
            marks.push((view.to_canvas(*pos), "E", super::GOLD));
        }
    }
    // Zones where a tracked quest still has work, for the tree.
    let quest_zones: Vec<String> = super::quests::tracked(&*sim, &track)
        .into_iter()
        .filter_map(|q| content.quest(&q.id).map(|d| (q, d)))
        .flat_map(|(q, d)| {
            d.objectives
                .iter()
                .enumerate()
                .filter(|(k, o)| q.counts.get(*k).copied().unwrap_or(0) < o.need())
                .map(|(_, o)| content.objective_zones(o))
                // "Anywhere" work (MeatProxies are on every floor) would tag the whole tree.
                .filter(|z| z.len() <= 2)
                .flatten()
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect();
    let doors: Vec<(Vec2, String, bool)> = sim
        .zone_links(&flow.zone)
        .into_iter()
        .filter_map(|l| {
            let (_, pos, _) = flow.anchors.iter().find(|a| a.0 == l.anchor)?;
            let open = l.unlocked || flow.unlock_all;
            // The hatch to the Resistance is not on the map until the Opening has played.
            if !open && combat_core::ilmari::is_ilmari_zone(&l.to) {
                return None;
            }
            Some((view.to_canvas(*pos), l.to, open))
        })
        .collect();
    let calendar = flow
        .anchors
        .iter()
        .find(|a| a.0 == super::minimap::CALENDAR_ANCHOR)
        .map(|a| view.to_canvas(a.1));
    let sig = format!(
        "{:?}|{:?}|{:?}|{quest_zones:?}|{calendar:?}{}",
        marks.iter().map(|m| (m.0.round(), m.1)).collect::<Vec<_>>(),
        doors.iter().map(|d| (&d.1, d.2)).collect::<Vec<_>>(),
        zone_tree(&*sim)
            .iter()
            .map(|(_, z)| (
                sim.zone_cleared(z),
                zone_state(&*sim, z, flow.unlock_all).is_ok()
            ))
            .collect::<Vec<_>>(),
        cal.news
    );
    if sig == view.shown {
        return;
    }
    view.shown = sig;
    for e in &markers {
        commands.entity(e).despawn();
    }
    commands.entity(canvas.0).with_children(|c| {
        // District names under everything else, so the big campus reads at a glance.
        let campus = flow.zone == "campus";
        if campus {
            for d in &crate::floors::DISTRICTS {
                let at = view.to_canvas(Vec3::new(d.centre.0, 0.0, d.centre.1));
                let w = d.name.len() as f32 * 5.6;
                c.spawn((
                    MapMarker,
                    label(d.name.to_uppercase(), 10.0),
                    TextColor(DISTRICT),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px((at.x - w / 2.0).clamp(0.0, CANVAS_W - w)),
                        top: px(at.y - 6.0),
                        ..default()
                    },
                    GlobalZIndex(34),
                ));
            }
        }
        for (at, glyph, color) in &marks {
            marker(c, *at, glyph, 15.0, *color);
        }
        // The Company Calendar: its icon, a "!" while something is unplayed, and its name.
        if let Some(at) = calendar {
            const S: f32 = 20.0;
            c.spawn((
                MapMarker,
                ImageNode::new(icons.get(&assets, "calendar")),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(at.x - S / 2.0),
                    top: px(at.y - S / 2.0),
                    width: px(S),
                    height: px(S),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                GlobalZIndex(36),
            ));
            let name = if cal.news { "! Calendar" } else { "Calendar" };
            c.spawn((
                MapMarker,
                label(name, 11.0),
                TextColor(super::GOLD),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(at.x - 32.0),
                    top: px(at.y + S / 2.0 + 1.0),
                    ..default()
                },
                GlobalZIndex(36),
            ));
        }
        for (at, to, open) in &doors {
            let color = if *open { OPEN } else { LOCKED };
            marker(c, *at, "#", 13.0, color);
            // On the campus the zone ids would overprint the district names.
            if campus {
                continue;
            }
            c.spawn((
                MapMarker,
                label(to.clone(), 10.0),
                TextColor(color),
                Node {
                    position_type: PositionType::Absolute,
                    left: px((at.x + 8.0).min(CANVAS_W - 80.0)),
                    top: px(at.y - 6.0),
                    ..default()
                },
                GlobalZIndex(35),
            ));
        }
    });
    commands.entity(*graph).despawn_related::<Children>();
    commands.entity(*graph).with_children(|g| {
        g.spawn((label("$ tree ~/campus", 12.0), TextColor(ACCENT)));
        // Open zones and the next locked step of each branch; Ilmari content only once reachable
        // (after the reveal), so a level-1 player never reads its names.
        let mut open_at = [false; 16];
        let mut more = 0;
        for (depth, z) in zone_tree(&*sim) {
            let here = z == flow.zone;
            let state = zone_state(&*sim, &z, flow.unlock_all);
            let ok = state.is_ok();
            let ilmari = combat_core::ilmari::is_ilmari_zone(&z);
            let parent_open = depth == 0 || open_at.get(depth - 1).copied().unwrap_or(false);
            if let Some(o) = open_at.get_mut(depth) {
                *o = ok;
            }
            if !(ok || (parent_open && !ilmari)) {
                more += usize::from(!ilmari);
                continue;
            }
            let (tag, color) = if here {
                ("you are here".to_string(), ACCENT)
            } else if sim.zone_cleared(&z) {
                ("cleared".to_string(), OPEN)
            } else {
                match &state {
                    Ok(()) => ("open".to_string(), Color::WHITE),
                    Err(_) => ("locked".to_string(), LOCKED),
                }
            };
            let name = content.zone(&z).map_or(z.as_str(), |d| d.name.as_str());
            // Indent by padding, not spaces, so a wrapped line keeps its depth.
            let indent = |extra: f32| Node {
                padding: UiRect::left(px(depth as f32 * 12.0 + extra)),
                ..default()
            };
            g.spawn((
                label(format!("{name}  [{tag}]"), 12.0),
                TextColor(color),
                indent(0.0),
            ));
            if quest_zones.contains(&z) {
                g.spawn((
                    label("! tracked objectives", 10.0),
                    TextColor(QUEST),
                    indent(12.0),
                ));
            }
            if let (Err(reason), false) = (state, here) {
                let reason = reason.trim_start_matches("Requires: ");
                g.spawn((
                    label(format!("needs: {reason}"), 10.0),
                    TextColor(DIM),
                    indent(12.0),
                ));
            }
        }
        if more > 0 {
            g.spawn((
                label(format!("+{more} more to unlock"), 10.0),
                TextColor(DIM),
            ));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tree_reaches_every_zone_once_and_starts_at_the_campus() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("content")
        };
        let t = zone_tree(&sim);
        assert_eq!(t[0], (0, "campus".to_string()));
        // Minigame venues are reached through their invite, not a door: not on the map.
        let c = sim.content();
        let doors = c
            .zones
            .iter()
            .filter(|z| !c.minigames.iter().any(|m| m.zone == z.id))
            .count();
        assert_eq!(t.len(), doors);
        assert!(zone_state(&sim, "datacenter/1", false).is_err());
        assert!(zone_state(&sim, "datacenter/1", true).is_ok());
        assert!(zone_state(&sim, "tower/1", false).is_ok());
    }

    #[test]
    fn the_campus_raster_has_floor_and_walls() {
        let Some(l) = build_zone("campus") else {
            panic!()
        };
        let (img, _, scale) = rasterise(&l);
        assert!(scale > 0.0);
        let data = img.data.unwrap_or_default();
        let walls = data.chunks(4).filter(|p| *p == WALL_RGBA).count();
        let floor = data.chunks(4).filter(|p| *p == FLOOR_RGBA).count();
        assert!(walls > 500 && floor > 10_000, "{walls} {floor}");
    }
}
