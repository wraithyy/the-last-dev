//! World-space nameplates: UI nodes pinned above every NPC's head. Hostiles get
//! a red name and an HP bar, friendlies a green name and their title; bosses and
//! mini-bosses carry a tag. Plates are culled beyond `PLATE_RANGE` and behind the camera.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{SimReady, SimUnit, SimView};
use crate::config::head_height;
use crate::copy;
use crate::hud::{ACCENT_WARM, BAR_BG, label, put};
use crate::player::Player;
use crate::targeting::Target;
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::EntityId;
use std::collections::HashMap;

const PLATE_W: f32 = 180.0;
const PLATE_RANGE: f32 = 40.0;
const PLATE_GAP: f32 = 0.3;
/// Plates solved and drawn per frame, nearest first (the target always sorts first).
const MAX_PLATES: usize = 30;
const BAR_W: f32 = 110.0;
/// Group members, as WoW's party blue, so a colleague never reads as an NPC.
pub const PARTY_BLUE: Color = Color::srgb(0.35, 0.65, 1.0);

#[derive(Component)]
struct Nameplate {
    anchor: Entity,
    sim: EntityId,
    height: f32,
    /// Eased upward push (window px) that keeps this plate off its neighbours.
    offset: f32,
    /// Eased scale: distance and crowding shrink it.
    k: f32,
    placed: bool,
}
#[derive(Component)]
struct PlateFill(EntityId);
/// Quest giver marker ("!" / "?") on top of a friendly plate.
#[derive(Component)]
struct QuestMark(String);
/// Name text and "LOOT" tag of a plate; `loot_fx` recolours and reveals them on lootable corpses.
#[derive(Component)]
pub struct PlateName(pub EntityId);
#[derive(Component)]
pub struct PlateLootTag(pub EntityId);
/// Subtitle of a friendly plate: the unit's title, or a player's worn achievement title.
#[derive(Component)]
struct PlateTitle(EntityId);

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlateTops>().add_systems(
            Update,
            (
                spawn_plates,
                recolor_plates,
                place_plates.in_set(PlatePlacement),
                // Quest state changes on accept/turn-in, not per frame; each call walks the quest
                // list per plate, so 4 Hz keeps the markers current at a fraction of the cost.
                update_marks.run_if(crate::hud::every(0.25)),
                update_titles.run_if(crate::hud::every(0.25)),
            )
                .chain()
                .in_set(SimReady),
        );
    }
}

fn spawn_plates(
    mut commands: Commands,
    pal: Res<crate::settings::Palette>,
    sim: SimView,
    added: Query<(Entity, &SimUnit), (Added<SimUnit>, With<Unit>, Without<Player>)>,
) {
    for (anchor, su) in &added {
        let Some(u) = sim.unit(su.0) else { continue };
        let party = sim.players().contains(&u.id);
        let role = crate::classes::role_of_unit(sim.content(), &u.key).filter(|_| party);
        let tag = if u.boss {
            Some(("BOSS", ACCENT_WARM))
        } else if copy::is_elite(&u.key) {
            Some(("ELITE", Color::srgb(0.8, 0.55, 1.0)))
        } else {
            role.map(|r| (r.tag(), crate::classes::role_color(r)))
        };
        let title = sim
            .content()
            .unit_def(&u.key)
            .map(|d| d.title.clone())
            .unwrap_or_default();
        let mut root = commands.spawn((
            Nameplate {
                anchor,
                sim: u.id,
                height: head_height(&u.key) + PLATE_GAP,
                offset: 0.0,
                k: 1.0,
                placed: false,
            },
            Node {
                position_type: PositionType::Absolute,
                width: px(PLATE_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2),
                ..default()
            },
            UiTransform::default(),
            Visibility::Hidden,
            GlobalZIndex(5),
        ));
        root.with_children(|p| {
            if !u.hostile {
                p.spawn((
                    QuestMark(u.key.clone()),
                    label("", 34.0),
                    TextColor(Color::NONE),
                ));
            }
            p.spawn(Node {
                column_gap: px(4),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                if let Some((text, color)) = tag {
                    row.spawn((
                        Node {
                            padding: UiRect::axes(px(3), px(0)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(color),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        children![(label(text, 9.0), TextColor(color))],
                    ));
                }
                row.spawn((
                    PlateName(u.id),
                    label(format!("{} {}", u.level, u.name), 13.0),
                    TextColor(if u.hostile {
                        pal.hostile
                    } else if party {
                        PARTY_BLUE
                    } else {
                        pal.friendly
                    }),
                ));
                if u.hostile {
                    row.spawn((
                        PlateLootTag(u.id),
                        Node {
                            display: Display::None,
                            padding: UiRect::axes(px(3), px(0)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(crate::loot_fx::GOLD),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        children![(label("LOOT", 9.0), TextColor(crate::loot_fx::GOLD))],
                    ));
                }
            });
            if u.hostile {
                p.spawn((
                    Node {
                        width: px(BAR_W),
                        height: px(7),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(2)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                    BackgroundColor(BAR_BG),
                ))
                .with_child((
                    PlateFill(u.id),
                    Node {
                        height: percent(100),
                        width: percent(100),
                        ..default()
                    },
                    BackgroundColor(pal.enemy_health),
                ));
            } else {
                p.spawn((
                    PlateTitle(u.id),
                    label(plate_title(&title), 10.0),
                    TextColor(Color::srgb(0.75, 0.85, 0.78)),
                ));
            }
        });
    }
}

/// A colourblind palette switch repaints the plates already up (living units only: corpse
/// names belong to `loot_fx`).
fn recolor_plates(
    pal: Res<crate::settings::Palette>,
    sim: SimView,
    mut names: Query<(&PlateName, &mut TextColor)>,
    mut fills: Query<(&PlateFill, &mut BackgroundColor)>,
) {
    if !pal.is_changed() || pal.is_added() {
        return;
    }
    for (n, mut c) in &mut names {
        let Some(u) = sim.unit(n.0).filter(|u| u.alive) else {
            continue;
        };
        if sim.players().contains(&u.id) {
            continue;
        }
        c.0 = if u.hostile { pal.hostile } else { pal.friendly };
    }
    for (_, mut bg) in &mut fills {
        bg.0 = pal.enemy_health;
    }
}

fn plate_title(title: &str) -> String {
    if title.is_empty() {
        String::new()
    } else {
        format!("<{title}>")
    }
}

/// Titles change when a player unlocks or picks an achievement, and a minigame may badge a
/// unit with something else for a while (the Reorg's names and roles).
fn update_titles(
    sim: SimView,
    badges: Res<crate::minigame::PlateBadges>,
    mut titles: Query<(&PlateTitle, &mut Text)>,
) {
    for (PlateTitle(id), mut text) in &mut titles {
        let Some(u) = sim.unit(*id) else { continue };
        let content = sim.content();
        let title = badges
            .0
            .get(id)
            .map(String::as_str)
            .or_else(|| content.title_text(&u.title))
            .or_else(|| content.unit_def(&u.key).map(|d| d.title.as_str()))
            .unwrap_or("");
        let want = plate_title(title);
        if text.0 != want {
            text.0 = want;
        }
    }
}

/// Screen rectangle of one plate before de-cluttering: bottom-centre anchor and size, window px.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlateBox {
    x: f32,
    /// Bottom edge, y grows downwards.
    y: f32,
    w: f32,
    h: f32,
}

/// Upward push (px, never negative) per plate so no two rectangles overlap. `boxes` must be
/// sorted by priority: the first plate never moves, later ones stack above whatever they hit.
fn solve_stacks(boxes: &[PlateBox], gap: f32) -> Vec<f32> {
    let mut offsets: Vec<f32> = Vec::with_capacity(boxes.len());
    for (i, p) in boxes.iter().enumerate() {
        let mut off = 0.0_f32;
        // Offsets only grow, so n passes always settle n plates.
        for _ in 0..=i {
            let mut moved = false;
            for (q, qoff) in boxes[..i].iter().zip(&offsets) {
                let apart_x = (p.x - q.x).abs() >= (p.w + q.w) / 2.0;
                let q_top = q.y - qoff - q.h;
                let p_bottom = p.y - off;
                let apart_y = p_bottom <= q_top - gap + 0.01 || p_bottom - p.h >= q.y - qoff + gap;
                if !apart_x && !apart_y {
                    // Stack above: with a shared anchor row the plate nearer the camera stays low.
                    off = p.y - q_top + gap;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        offsets.push(off);
    }
    offsets
}

/// Sort key: the target first, then nearest first in 2 m bands. Banding stops two units at
/// almost the same distance from swapping places every frame; the entity index breaks ties.
fn priority_key(targeted: bool, dist: f32, entity: Entity) -> (i32, u32) {
    let band = if targeted {
        i32::MIN
    } else {
        (dist / 2.0) as i32
    };
    (band, entity.index_u32())
}

/// Plates whose pushed-up offset exceeds this many plate heights are shrunk, then hidden.
const SHRINK_AT: f32 = 1.5;
const HIDE_AT: f32 = 3.5;
/// Exponential rates (1/s) of the eased offset and scale.
const OFFSET_RATE: f32 = 12.0;
const SCALE_RATE: f32 = 10.0;
/// Plate text is narrower than the `PLATE_W` box that holds it.
const CONTENT_W: f32 = 130.0;
const STACK_GAP_PX: f32 = 2.0;
/// Width used to test a plate against HUD panels and screen edges: a wrapped title can run
/// wider than `CONTENT_W`.
const SHOW_W: f32 = 160.0;

/// Screen y (window px, downwards) of the top edge of every visible plate, by sim id. Yell
/// bubbles read it so they sit above the plate's final position rather than the raw head point.
#[derive(Resource, Default)]
pub struct PlateTops(pub HashMap<EntityId, f32>);

/// Plate placement runs here; consumers that anchor to plates order themselves after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlatePlacement;

struct Candidate {
    entity: Entity,
    key: (i32, u32),
    base_k: f32,
    targeted: bool,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn place_plates(
    mut commands: Commands,
    sim: SimView,
    time: Res<Time>,
    target: Res<Target>,
    ui_scale: Res<UiScale>,
    hud: Res<crate::hud::HudRects>,
    mut tops: ResMut<PlateTops>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    transforms: Query<(&Transform, Option<&InheritedVisibility>), With<SimUnit>>,
    mut cands: Local<Vec<Candidate>>,
    mut plates: Query<(
        Entity,
        &mut Nameplate,
        &ComputedNode,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        &mut GlobalZIndex,
    )>,
    mut fills: Query<(&PlateFill, &mut Node), Without<Nameplate>>,
) {
    let (cam, cam_tf) = *camera;
    // Node coordinates are multiplied by UiScale, viewport coordinates are not.
    let scale = ui_scale.0.max(0.01);
    let eye = cam_tf.translation();
    let forward = cam_tf.forward();
    // The camera's target, which is the offscreen image during screenshots.
    let view = cam
        .logical_viewport_size()
        .unwrap_or(Vec2::new(window.width(), window.height()));
    let (view_w, view_h) = (view.x, view.y);
    let dt = time.delta_secs();
    tops.0.clear();
    cands.clear();
    for (e, plate, computed, _, _, mut vis, _) in &mut plates {
        let Ok((tf, inherited)) = transforms.get(plate.anchor) else {
            commands.entity(e).despawn();
            continue;
        };
        // A unit hidden on purpose (cutscene, pooled) must not leave its plate floating.
        if inherited.is_some_and(|v| !v.get()) {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        // A lootable corpse keeps its plate so the gold name marks it.
        let alive = sim.unit(plate.sim).is_some_and(|u| u.alive || u.lootable);
        let world = tf.translation + Vec3::Y * plate.height;
        let dist = world.distance(eye);
        let in_front = (world - eye).dot(*forward) > 0.0;
        let screen = cam.world_to_viewport(cam_tf, world);
        let Some(screen) = screen
            .ok()
            .filter(|_| alive && in_front && dist <= PLATE_RANGE)
        else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let targeted = target.0 == Some(plate.anchor);
        // Far plates shrink so a whole pack stays readable; the target stands out.
        let base_k = (1.1 - dist / 60.0).clamp(0.6, 1.0) * if targeted { 1.15 } else { 1.0 };
        // Physical size over the window factor is window px; before the first layout pass a
        // rough size keeps the plate from stacking on top of itself for a frame.
        let h = if computed.size().y > 1.0 {
            computed.size().y / window.scale_factor()
        } else {
            30.0 * scale
        };
        cands.push(Candidate {
            entity: e,
            key: priority_key(targeted, dist, e),
            base_k,
            targeted,
            x: screen.x,
            y: screen.y,
            w: CONTENT_W * scale * base_k,
            h: h * base_k,
        });
    }

    cands.sort_by_key(|c| c.key);
    // Beyond the nearest few dozen a plate is unreadable anyway, and the overlap solver is
    // quadratic in the plate count.
    let keep = MAX_PLATES.min(cands.len());
    for c in cands.drain(keep..) {
        if let Ok((_, _, _, _, _, mut vis, _)) = plates.get_mut(c.entity) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    let boxes: Vec<PlateBox> = cands
        .iter()
        .map(|c| PlateBox {
            x: c.x,
            y: c.y,
            w: c.w,
            h: c.h,
        })
        .collect();
    let goals = solve_stacks(&boxes, STACK_GAP_PX * scale);
    let off_ease = 1.0 - (-OFFSET_RATE * dt).exp();
    let k_ease = 1.0 - (-SCALE_RATE * dt).exp();
    for (c, goal) in cands.iter().zip(goals) {
        let Ok((_, mut plate, _, mut node, mut tf_ui, mut vis, mut z)) = plates.get_mut(c.entity)
        else {
            continue;
        };
        // The target is placed first, so it is never pushed; everyone else may crowd out.
        let crowd = if c.targeted { 0.0 } else { goal / c.h.max(1.0) };
        // A plate that would draw over a HUD panel (minimap, frames, bars, logs) or be cut by the
        // screen edge is not shown.
        let half_w = SHOW_W * scale * c.base_k / 2.0;
        let rect = Rect {
            min: Vec2::new(c.x - half_w, c.y - goal - c.h),
            max: Vec2::new(c.x + half_w, c.y - goal),
        };
        let off_screen = rect.min.x < 0.0 || rect.max.x > view_w || rect.min.y < 0.0;
        let want_k = if crowd > HIDE_AT || off_screen || hud.hits(rect) {
            0.0
        } else if crowd > SHRINK_AT {
            c.base_k * 0.75
        } else {
            c.base_k
        };
        if plate.placed {
            plate.offset += (goal - plate.offset) * off_ease;
            plate.k += (want_k - plate.k) * k_ease;
        } else {
            plate.offset = goal;
            plate.k = want_k;
            plate.placed = true;
        }
        if plate.k < 0.05 {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        // The target's plate is drawn over any plate stacked next to it.
        z.set_if_neq(GlobalZIndex(if c.targeted { 6 } else { 5 }));
        put!(node.left, px(c.x / scale - PLATE_W / 2.0));
        put!(node.bottom, px((view_h - c.y + plate.offset) / scale));
        put!(tf_ui.scale, Vec2::splat(plate.k));
        // Scaling is about the centre, so the visible top edge sits half the shrink lower.
        let h_unscaled = c.h / c.base_k;
        let top = c.y - plate.offset - h_unscaled * (1.0 + plate.k) / 2.0;
        tops.0.insert(plate.sim, top);
    }
    for (fill, mut node) in &mut fills {
        let pct = sim
            .unit(fill.0)
            .map_or(0.0, |u| 100.0 * u.hp as f32 / u.max_hp.max(1) as f32);
        let w = percent(pct);
        if node.width != w {
            node.width = w;
        }
    }
}

fn update_marks(
    sim: SimView,
    flow: Res<crate::game::GameFlow>,
    mut marks: Query<(&QuestMark, &mut Text, &mut TextColor)>,
) {
    for (mark, mut text, mut color) in &mut marks {
        let (glyph, c) =
            crate::hud::giver_mark(&*sim, &mark.0, &flow.zone).unwrap_or(("", Color::NONE));
        crate::hud::set_text(&mut text, glyph);
        if color.0 != c {
            color.0 = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f32, y: f32) -> PlateBox {
        PlateBox {
            x,
            y,
            w: 100.0,
            h: 30.0,
        }
    }

    fn overlap(a: &PlateBox, ao: f32, c: &PlateBox, co: f32) -> bool {
        let (ay, cy) = (a.y - ao, c.y - co);
        (a.x - c.x).abs() < (a.w + c.w) / 2.0 && ay - a.h < cy && cy - c.h < ay
    }

    #[test]
    fn separate_plates_stay_put() {
        let offs = solve_stacks(&[b(0.0, 300.0), b(400.0, 300.0), b(0.0, 100.0)], 2.0);
        assert_eq!(offs, vec![0.0; 3]);
    }

    #[test]
    fn first_plate_never_moves_and_the_rest_stack_above() {
        let boxes = [
            b(500.0, 300.0),
            b(505.0, 300.0),
            b(498.0, 305.0),
            b(510.0, 296.0),
        ];
        let offs = solve_stacks(&boxes, 2.0);
        assert_eq!(offs[0], 0.0);
        for i in 0..boxes.len() {
            for j in 0..i {
                assert!(
                    !overlap(&boxes[i], offs[i], &boxes[j], offs[j]),
                    "{i} overlaps {j}: {offs:?}"
                );
            }
        }
        // Stacking only ever pushes up.
        assert!(offs.iter().all(|o| *o >= 0.0));
        assert!(offs[1] > 0.0 && offs[2] > offs[1] - 40.0);
    }

    #[test]
    fn solving_is_deterministic_so_frames_do_not_jitter() {
        let boxes = [b(500.0, 300.0), b(520.0, 310.0), b(480.0, 290.0)];
        assert_eq!(solve_stacks(&boxes, 2.0), solve_stacks(&boxes, 2.0));
    }

    #[test]
    fn target_sorts_first_and_distance_bands_are_stable() {
        let e = Entity::from_raw_u32(9).unwrap();
        let f = Entity::from_raw_u32(3).unwrap();
        assert!(priority_key(true, 50.0, e) < priority_key(false, 1.0, f));
        // 10.1 m and 10.9 m share a band: ordering falls to the entity index, not distance.
        assert!(priority_key(false, 10.9, f) < priority_key(false, 10.1, e));
        assert!(priority_key(false, 3.0, e) < priority_key(false, 12.0, f));
    }
}
