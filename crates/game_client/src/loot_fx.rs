//! Lootable-corpse feedback: rising golden sparkles, a gold nameplate with a
//! LOOT tag, and the hover cursor (crosshair / grab / pointer). Also the corpse decay: a
//! corpse sinks into the floor once its timer (see `combat_core::CORPSE_EMPTY_MS`) runs out.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::too_many_arguments)]

use crate::arena::{Colliders, FlatDecor};
use crate::combat::{CombatEvent, SimRead, SimReady, SimUnit, SimView};
use crate::input::{MouseState, Paused};
use crate::nameplates::{PlateLootTag, PlateName};
use crate::player::{Player, PlayerBody};
use crate::targeting::{Target, pick_unit};
use crate::touch::TouchMode;
use crate::units::Unit;
use bevy::prelude::*;
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage, PrimaryWindow};
use combat_core::{CORPSE_EMPTY_MS, CORPSE_LOOT_MS, EntityId, Event, TICK_HZ, ms_to_ticks};
use std::collections::{HashMap, HashSet};

pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.25);
const SPARKLE_CAP: usize = 40;
const SPARKLE_RANGE: f32 = 35.0;
const SPARKLE_EVERY: f32 = 0.14;
const SPARKLE_LIFE: f32 = 1.3;
/// Re-pick at least this often even with a still cursor: units walk under it.
const PICK_EVERY: f32 = 0.15;
/// A decaying corpse sinks this deep over the last `SINK_SECS` of its timer. Sinking under the
/// opaque floor instead of an alpha fade keeps the shared character materials untouched (and
/// WebGL2 has no order-independent transparency to fade skinned meshes cleanly anyway).
const SINK_M: f32 = 0.8;
const SINK_SECS: f32 = 3.0;
/// Horizontal reach of a lying body around its root, for finding the decor it rests on.
const CORPSE_RADIUS: f32 = 0.7;
/// Gap above the surface so the lying mesh never z-fights with floor or rug.
const CORPSE_CLEARANCE: f32 = 0.015;

/// Height a corpse at `pos` rests at: on top of the rug or carpet under it, not inside it.
fn rest_lift(decor: &FlatDecor, pos: Vec3) -> f32 {
    // Decor tops are absolute; the corpse already lies on the ground at `pos.y`.
    (decor.top_near(pos, CORPSE_RADIUS) - pos.y).max(0.0) + CORPSE_CLEARANCE
}

/// This player's view of a corpse's timer, mirroring the sim's `refresh_corpse`: 3 minutes
/// from the death while it holds loot for us, 30 s once it holds none. Online a teammate's roll
/// keeps the corpse in the server's sim; it still goes here. Offline the sim despawns it on the
/// same tick.
#[derive(Component)]
struct CorpseClock {
    died: u64,
    empty_since: Option<u64>,
}

impl CorpseClock {
    fn end(&self) -> u64 {
        let last = self.died + ms_to_ticks(CORPSE_LOOT_MS);
        self.empty_since
            .map_or(last, |t| (t + ms_to_ticks(CORPSE_EMPTY_MS)).min(last))
    }
}

/// How far under the floor a corpse is with `left` seconds on its timer (eased in).
fn sink_depth(left: f32) -> f32 {
    let k = (1.0 - left / SINK_SECS).clamp(0.0, 1.0);
    SINK_M * k * k
}

/// Corpses whose drop was empty: nothing to show, though the sim still calls them lootable.
#[derive(Resource, Default)]
struct EmptyCorpses(HashSet<EntityId>);

#[derive(Resource)]
struct SparkleAssets {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Sparkle {
    corpse: EntityId,
    origin: Vec3,
    age: f32,
    phase: f32,
}

pub struct LootFxPlugin;

impl Plugin for LootFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EmptyCorpses>()
            .add_systems(Startup, (setup, load_cursors))
            .add_systems(
                Update,
                (
                    track_empty,
                    decay_corpses,
                    spawn_sparkles,
                    update_sparkles,
                    mark_plates,
                    hover_cursor,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(SparkleAssets {
        mesh: meshes.add(Sphere::new(0.5)),
        mat: mats.add(StandardMaterial {
            // Unlit ignores emissive, so the colour goes in base_color.
            base_color: GOLD,
            unlit: true,
            ..default()
        }),
    });
}

fn track_empty(
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    mut empty: ResMut<EmptyCorpses>,
) {
    for CombatEvent(e) in events.read() {
        if let Event::LootDropped { corpse, items, sp } = e
            && items.is_empty()
            && *sp == 0
        {
            empty.0.insert(*corpse);
        }
    }
    empty
        .0
        .retain(|id| sim.unit(*id).is_some_and(|u| u.lootable));
}

type CorpseParts = (
    Entity,
    &'static SimUnit,
    &'static mut Transform,
    &'static mut Visibility,
    Option<&'static mut CorpseClock>,
    Has<Unit>,
);

/// Runs after the render interpolation wrote this frame's pose, so the sink offset never
/// accumulates. A sunk corpse loses its `Unit` mirror: no picking, no target frame (the target
/// drops), no plate; a unit that comes back to life gets it again.
fn decay_corpses(
    mut commands: Commands,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    decor: Res<FlatDecor>,
    mut target: ResMut<Target>,
    mut corpses: Query<CorpseParts, Without<Player>>,
) {
    let now = sim.now().0;
    for (e, su, mut tf, mut vis, clock, mirrored) in &mut corpses {
        let Some(u) = sim.unit(su.0) else { continue };
        if !u.alive {
            // Bosses and respawners keep their corpse too, so the lift is not tied to the sink.
            tf.translation.y += rest_lift(&decor, tf.translation);
        }
        // Bosses are set pieces, respawning units come back in place, players get resurrected.
        let keeps = u.boss || u.respawn_in_ms.is_some() || sim.players().contains(&u.id);
        if u.alive || keeps {
            if clock.is_some() {
                commands.entity(e).remove::<CorpseClock>();
                *vis = Visibility::Inherited;
                if !mirrored {
                    commands.entity(e).insert(Unit {
                        hostile: u.hostile,
                        alive: u.alive,
                        lootable: false,
                        fighting: false,
                    });
                }
            }
            continue;
        }
        let Some(mut clock) = clock else {
            commands.entity(e).insert(CorpseClock {
                died: now,
                empty_since: None,
            });
            continue;
        };
        if clock.empty_since.is_none() && !has_loot(&*sim, &empty, su.0) {
            clock.empty_since = Some(now);
        }
        let left = clock.end().saturating_sub(now) as f32 / TICK_HZ as f32;
        tf.translation.y -= sink_depth(left);
        if left <= 0.0 && mirrored {
            *vis = Visibility::Hidden;
            commands.entity(e).remove::<Unit>();
            if target.0 == Some(e) {
                target.0 = None;
            }
        }
    }
}

fn has_loot(sim: &dyn SimRead, empty: &EmptyCorpses, id: EntityId) -> bool {
    !empty.0.contains(&id) && sim.unit(id).is_some_and(|u| u.lootable)
}

fn spawn_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<SparkleAssets>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    player: Single<&PlayerBody, With<Player>>,
    corpses: Query<(&Transform, &SimUnit), With<Unit>>,
    live: Query<(), With<Sparkle>>,
    mut clock: Local<(f32, u32)>,
) {
    clock.0 += time.delta_secs();
    if clock.0 < SPARKLE_EVERY {
        return;
    }
    clock.0 = 0.0;
    let mut room = SPARKLE_CAP.saturating_sub(live.iter().count());
    for (tf, su) in &corpses {
        if room == 0 {
            break;
        }
        if !has_loot(&*sim, &empty, su.0) || tf.translation.distance(player.pos) > SPARKLE_RANGE {
            continue;
        }
        clock.1 = clock.1.wrapping_add(1);
        // Golden-ratio phase spreads sparkles around the corpse without an RNG.
        let phase = clock.1 as f32 * 2.399_963;
        commands.spawn((
            Sparkle {
                corpse: su.0,
                origin: tf.translation + Vec3::Y * 0.2,
                age: 0.0,
                phase,
            },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.mat.clone()),
            Transform::from_translation(tf.translation).with_scale(Vec3::ZERO),
        ));
        room -= 1;
    }
}

fn update_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    mut q: Query<(Entity, &mut Sparkle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf) in &mut q {
        s.age += dt;
        let k = s.age / SPARKLE_LIFE;
        if k >= 1.0 || !has_loot(&*sim, &empty, s.corpse) {
            commands.entity(e).despawn();
            continue;
        }
        let a = s.phase + s.age * 1.6;
        let r = 0.25 + 0.45 * (s.phase * 1.7).sin().abs();
        tf.translation = s.origin + Vec3::new(a.cos() * r, k * 1.9, a.sin() * r);
        // Scale envelope stands in for alpha fade so all sparkles share one material.
        let pulse = 0.75 + 0.25 * (s.age * 12.0 + s.phase).sin();
        tf.scale = Vec3::splat(0.09 * (k * std::f32::consts::PI).sin() * pulse);
    }
}

fn mark_plates(
    sim: SimView,
    empty: Res<EmptyCorpses>,
    mut names: Query<(&PlateName, &mut TextColor)>,
    mut tags: Query<(&PlateLootTag, &mut Node)>,
) {
    for (n, mut color) in &mut names {
        if has_loot(&*sim, &empty, n.0) {
            color.set_if_neq(TextColor(GOLD));
        }
    }
    for (t, mut node) in &mut tags {
        let d = if has_loot(&*sim, &empty, t.0) {
            Display::Flex
        } else {
            Display::None
        };
        node.set_if_neq(Node {
            display: d,
            ..node.clone()
        });
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hover {
    Nothing,
    Ui,
    Hostile,
    Friendly,
    Prop,
    Corpse { in_reach: bool },
}

/// The Kenney cursor shown for a hover (assets/cursors).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Pointer {
    Normal,
    Ui,
    Talk,
    Loot,
    Attack,
    Use,
    NotAllowed,
}

impl Pointer {
    const ALL: [Pointer; 7] = [
        Pointer::Normal,
        Pointer::Ui,
        Pointer::Talk,
        Pointer::Loot,
        Pointer::Attack,
        Pointer::Use,
        Pointer::NotAllowed,
    ];

    /// File under assets/ and the hotspot in its 32x32 pixels (the arrow tip or the glyph centre).
    fn art(self) -> (&'static str, (u16, u16)) {
        match self {
            Pointer::Normal => ("cursors/pointer_a.png", (9, 7)),
            Pointer::Ui => ("cursors/hand_point.png", (11, 4)),
            Pointer::Talk => ("cursors/message_dots_round.png", (16, 16)),
            Pointer::Loot => ("cursors/hand_open.png", (16, 16)),
            Pointer::Attack => ("cursors/tool_sword_a.png", (5, 5)),
            Pointer::Use => ("cursors/cursor_cogs.png", (4, 3)),
            Pointer::NotAllowed => ("cursors/cursor_disabled.png", (2, 2)),
        }
    }
}

fn cursor_for(h: Hover) -> Pointer {
    match h {
        Hover::Nothing => Pointer::Normal,
        Hover::Ui => Pointer::Ui,
        Hover::Friendly => Pointer::Talk,
        Hover::Hostile => Pointer::Attack,
        Hover::Prop => Pointer::Use,
        Hover::Corpse { in_reach: true } => Pointer::Loot,
        Hover::Corpse { in_reach: false } => Pointer::NotAllowed,
    }
}

#[derive(Resource)]
struct Pointers(HashMap<Pointer, Handle<Image>>);

fn load_cursors(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Pointers(
        Pointer::ALL
            .iter()
            .map(|p| (*p, assets.load(p.art().0)))
            .collect(),
    ));
}

fn hover_cursor(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    sim: SimView,
    empty: Res<EmptyCorpses>,
    colliders: Res<Colliders>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    player: Single<&PlayerBody, With<Player>>,
    ui: Query<&Interaction>,
    units: Query<(Entity, &Transform, &Unit)>,
    sims: Query<&SimUnit>,
    (prop, props): (Res<crate::office::PropHover>, Res<crate::office::Props>),
    (pointers, images, touch): (Res<Pointers>, Res<Assets<Image>>, Res<TouchMode>),
    mut state: Local<(Option<Vec2>, f32, Option<Pointer>)>,
) {
    // Fingers have no pointer to draw.
    if touch.active {
        return;
    }
    let (win_entity, win) = *window;
    // While steering the pointer is locked and hidden; leave the icon alone.
    if mouse.right || mouse.left {
        return;
    }
    let cursor = win.cursor_position();
    state.1 += time.delta_secs();
    if cursor == state.0 && state.1 < PICK_EVERY {
        return;
    }
    state.0 = cursor;
    state.1 = 0.0;
    let hover = match cursor {
        _ if paused.0 => Hover::Nothing,
        None => Hover::Nothing,
        Some(_) if ui.iter().any(|i| *i != Interaction::None) => Hover::Ui,
        Some(c) => pick_unit(c, *camera, &colliders.0, &units, 0.0)
            .and_then(|e| {
                let (_, tf, unit) = units.get(e).ok()?;
                let id = sims.get(e).ok()?.0;
                Some(if unit.alive {
                    if unit.hostile {
                        Hover::Hostile
                    } else {
                        Hover::Friendly
                    }
                } else if has_loot(&*sim, &empty, id) {
                    let reach = sim.content().config.loot_range_m;
                    Hover::Corpse {
                        in_reach: tf.translation.distance(player.pos) <= reach,
                    }
                } else {
                    Hover::Nothing
                })
            })
            // A quest pickup takes the loot hand, like a corpse with something on it.
            .unwrap_or(match prop.0.and_then(|i| props.list.get(i)) {
                Some(p) if p.kind == combat_core::PropKind::Pickup => Hover::Corpse {
                    in_reach: p.stand.distance(player.pos) <= p.reach,
                },
                Some(_) => Hover::Prop,
                None => Hover::Nothing,
            }),
    };
    let pointer = cursor_for(hover);
    if state.2 != Some(pointer) {
        let Some(handle) = pointers.0.get(&pointer).filter(|h| images.contains(h.id())) else {
            // Not decoded yet: keep the OS arrow and retry on the next pick.
            state.0 = None;
            return;
        };
        state.2 = Some(pointer);
        commands
            .entity(win_entity)
            .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
                handle: handle.clone(),
                hotspot: pointer.art().1,
                ..default()
            })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_maps_to_expected_cursor() {
        use Pointer::*;
        assert_eq!(cursor_for(Hover::Nothing), Normal);
        assert_eq!(cursor_for(Hover::Ui), Ui);
        assert_eq!(cursor_for(Hover::Friendly), Talk);
        assert_eq!(cursor_for(Hover::Hostile), Attack);
        assert_eq!(cursor_for(Hover::Prop), Use);
        assert_eq!(cursor_for(Hover::Corpse { in_reach: true }), Loot);
        assert_eq!(cursor_for(Hover::Corpse { in_reach: false }), NotAllowed);
    }

    #[test]
    fn every_pointer_has_a_distinct_existing_image_with_its_hotspot_inside() {
        let mut seen = HashSet::new();
        for p in Pointer::ALL {
            let (file, (x, y)) = p.art();
            assert!(seen.insert(file), "{file} reused");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets")
                .join(file);
            assert!(path.exists(), "{file}");
            // Cursor Pack PNGs are 32x32; a hotspot outside makes winit reject the cursor.
            assert!(x < 32 && y < 32, "{file}");
        }
    }

    #[test]
    fn corpse_rests_on_the_rug_and_only_half_on_it_too() {
        use crate::arena::DecorRect;
        let rug = DecorRect {
            center: Vec2::new(10.0, 0.0),
            half: Vec2::new(2.0, 1.0),
            yaw: 0.0,
            top: 0.024,
            round: false,
        };
        let decor = FlatDecor(vec![rug]);
        let on = rest_lift(&decor, Vec3::new(10.0, 0.0, 0.0));
        assert!(on > rug.top, "{on}");
        // Root just off the rug edge but the body reaches over it.
        assert!(rest_lift(&decor, Vec3::new(12.3, 0.0, 0.0)) > rug.top);
        assert_eq!(
            rest_lift(&decor, Vec3::new(20.0, 0.0, 0.0)),
            CORPSE_CLEARANCE
        );
        // The sink starts only at the end of the timer, so the lifted pose is the resting one.
        assert_eq!(sink_depth(SINK_SECS + 1.0), 0.0);
    }

    #[test]
    fn corpse_clock_matches_the_sim_rule_and_sinks_only_at_the_end() {
        let t = |ms| ms_to_ticks(ms);
        let held = CorpseClock {
            died: 100,
            empty_since: None,
        };
        assert_eq!(held.end(), 100 + t(CORPSE_LOOT_MS));
        let looted = CorpseClock {
            died: 100,
            empty_since: Some(400),
        };
        assert_eq!(looted.end(), 400 + t(CORPSE_EMPTY_MS));
        // Emptied just before the long timer: the long timer still wins.
        let late = CorpseClock {
            died: 100,
            empty_since: Some(100 + t(CORPSE_LOOT_MS) - 30),
        };
        assert_eq!(late.end(), 100 + t(CORPSE_LOOT_MS));
        assert_eq!(sink_depth(SINK_SECS + 1.0), 0.0);
        assert!(sink_depth(SINK_SECS / 2.0) > 0.0 && sink_depth(SINK_SECS / 2.0) < SINK_M);
        assert_eq!(sink_depth(0.0), SINK_M);
    }
}
