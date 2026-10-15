//! `~/radar`: the corner minimap. North up, the player in the middle. The zone outline is the
//! big map's raster, drawn once per zone and slid under a clipped window, so a frame only moves
//! a handful of UI nodes (a pool of dots) and never renders to a texture.

use super::map::{CANVAS_H, CANVAS_W, rasterise};
use super::{ACCENT, ACCENT_WARM, DIM, PANEL_EDGE, giver_mark, label, set_text};
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
        .add_systems(Update, update_radar.in_set(SimReady));
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

fn spawn_radar(mut commands: Commands) {
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
    (mut zone_text, mut coords): (
        Single<&mut Text, (With<RadarZone>, Without<RadarGlyph>, Without<RadarCoords>)>,
        Single<&mut Text, (With<RadarCoords>, Without<RadarGlyph>, Without<RadarZone>)>,
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

    let me = Vec2::new(player.pos.x, player.pos.z);
    let px_per_m = (MM / 2.0) / RADII[radar.zoom];
    // The raster is CANVAS_W x CANVAS_H canvas px at `scale` px per metre.
    let k = px_per_m / radar.scale;
    let at = to_view(radar.lo, me, px_per_m);
    image.1.width = px(CANVAS_W * k);
    image.1.height = px(CANVAS_H * k);
    image.1.left = px(MM / 2.0 + at.x);
    image.1.top = px(MM / 2.0 + at.y);

    // Facing is (-sin yaw, -cos yaw) in (x, z); the arrow art points at -z.
    arrow.rotation = Rot2::radians(-player.yaw);
    set_text(
        &mut coords,
        &format!("x {:>4.0}  z {:>4.0}", player.pos.x, player.pos.z),
    );

    let half = RADII[radar.zoom];
    let in_view = |p: Vec2| (p - me).abs().max_element() <= half;
    let mut marks: Vec<(Vec2, Kind)> = Vec::new();
    let wanted = super::quests::objective_units(&*sim, &track);
    for u in sim.units() {
        let p = Vec2::new(u.pos.x, u.pos.z);
        if Some(u.id) == sim.player() || !in_view(p) {
            continue;
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

    let mut shown: [Option<(&'static str, Color)>; POOL] = [None; POOL];
    for (dot, mut node, mut bg, mut border, mut tf) in &mut dots {
        let Some((p, kind)) = marks.get(dot.0) else {
            if node.display != Display::None {
                node.display = Display::None;
            }
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
        node.display = Display::Flex;
        node.width = px(size);
        node.height = px(size);
        node.left = px(MM / 2.0 + v.x - size / 2.0);
        node.top = px(MM / 2.0 + v.y - size / 2.0);
        node.border = if glyph.is_some() {
            UiRect::ZERO
        } else {
            UiRect::all(px(1))
        };
        node.border_radius = if turn == 0.0 && glyph.is_none() {
            BorderRadius::MAX
        } else {
            BorderRadius::ZERO
        };
        bg.0 = fill;
        *border = BorderColor::all(edge);
        tf.rotation = Rot2::degrees(turn);
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
    fn zoom_steps_clamp_to_the_table() {
        assert_eq!(step_zoom(0, -1), 0);
        assert_eq!(step_zoom(DEFAULT_ZOOM, 1), DEFAULT_ZOOM + 1);
        assert_eq!(step_zoom(RADII.len() - 1, 1), RADII.len() - 1);
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
