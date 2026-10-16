//! Target selection (Tab, click, Escape) and the data the target frame shows.

use crate::arena::Colliders;
use crate::collision::to_core;
use crate::combat::{CombatSim, SimUnit, SimView};
use crate::config::*;
use crate::input::{MouseState, Paused, QuestWindows, TrophiesOpen};
use crate::keybinds::{Action, Keybinds};
use crate::player::{Player, PlayerBody};
use crate::units::Unit;
use bevy::prelude::*;
use combat_core::{
    Aabb, EntityId, TAB_TARGET_RANGE, TargetCandidate, TargetError, line_of_sight, tab_target,
    validate_target,
};
use std::f32::consts::FRAC_PI_2;

#[derive(Resource, Default)]
pub struct Target(pub Option<Entity>);

/// Geometry checks against the current target, recomputed every frame for the HUD.
pub struct TargetReadout {
    pub spell_range: f32,
    pub distance: f32,
    pub in_range: bool,
    pub facing: bool,
    pub line_of_sight: bool,
}

#[derive(Resource, Default)]
pub struct TargetInfo(pub Option<TargetReadout>);

#[derive(Component)]
struct TargetRing {
    hostile: Handle<StandardMaterial>,
    friendly: Handle<StandardMaterial>,
}

pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Target>()
            .init_resource::<TargetInfo>()
            .add_systems(Startup, spawn_ring)
            .add_systems(
                Update,
                (
                    drop_invalid_target,
                    handle_keys,
                    party_keys,
                    click_select,
                    update_readout,
                    update_ring,
                )
                    .chain(),
            );
    }
}

fn spawn_ring(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut ring_material = |color: Color| {
        materials.add(StandardMaterial {
            // Unlit ignores emissive, so the colour goes in base_color.
            base_color: color,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    };
    let hostile = ring_material(Color::srgb(1.0, 0.05, 0.05));
    let friendly = ring_material(Color::srgb(0.2, 1.0, 0.3));
    commands.spawn((
        TargetRing {
            hostile: hostile.clone(),
            friendly,
        },
        Mesh3d(meshes.add(Annulus::new(RING_INNER, RING_OUTER))),
        MeshMaterial3d(hostile),
        Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        Visibility::Hidden,
    ));
}

/// A despawned target is dropped, not left dangling. Dead targets stay
/// selected on purpose: the corpse is what you loot (or the teammate you resurrect).
/// The player's own body (F1) has no `Unit` but is a valid target.
fn drop_invalid_target(
    mut target: ResMut<Target>,
    units: Query<&Unit>,
    player: Single<Entity, With<Player>>,
) {
    if let Some(e) = target.0
        && !units.contains(e)
        && e != *player
    {
        target.0 = None;
    }
}

/// Party keys (F1 yourself, F2-F5 the other group members in join order, as in WoW) are
/// `Action::Party(0..5)`; see `party_slot` for the mapping.
///
/// Sim id behind party key `slot`: 0 is `you`, `k` the k-th other member.
pub fn party_slot(players: &[EntityId], you: EntityId, slot: usize) -> Option<EntityId> {
    if slot == 0 {
        return Some(you);
    }
    players.iter().copied().filter(|p| *p != you).nth(slot - 1)
}

/// Bevy entity showing sim unit `id` (the player's own body included).
pub fn entity_of_sim(units: &Query<(Entity, &SimUnit)>, id: EntityId) -> Option<Entity> {
    units.iter().find(|(_, s)| s.0 == id).map(|(e, _)| e)
}

fn party_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    sim: SimView,
    me: Single<&SimUnit, With<Player>>,
    units: Query<(Entity, &SimUnit)>,
    mut target: ResMut<Target>,
) {
    if paused.0 {
        return;
    }
    let Some(slot) = (0..5u8).position(|i| binds.just(&keys, Action::Party(i))) else {
        return;
    };
    if let Some(e) = party_slot(sim.players(), me.0, slot).and_then(|id| entity_of_sim(&units, id))
    {
        target.0 = Some(e);
    }
}

#[allow(clippy::type_complexity)] // Bevy system parameters
fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    // Grouped because Esc closes these in priority order.
    (mut paused, mut trophies, mut windows, mut sprint, mut career, mut endgame, mut minigame): (
        ResMut<Paused>,
        ResMut<TrophiesOpen>,
        ResMut<QuestWindows>,
        ResMut<crate::input::SprintWindows>,
        ResMut<crate::input::CareerOpen>,
        ResMut<crate::input::EndgameOpen>,
        ResMut<crate::input::MinigameWindows>,
    ),
    mut target: ResMut<Target>,
    player: Single<&PlayerBody, With<Player>>,
    units: Query<(Entity, &Transform, &Unit, Option<&SimUnit>)>,
    (sim, mg_hud): (SimView, Res<crate::minigame::MinigameHud>),
) {
    if keys.just_pressed(KeyCode::Escape) {
        if minigame.close_one()
            || sprint.close_one()
            || windows.close_one()
            || std::mem::take(&mut trophies.0)
            || std::mem::take(&mut career.0)
            || std::mem::take(&mut endgame.0)
        {
            // The window closes first; the next Esc clears the target or pauses.
        } else if mg_hud.active && !paused.0 {
            // Inside a minigame Esc means "get me out", with a confirm (the frame's rule).
            minigame.leave = true;
        } else if target.0.take().is_none() {
            paused.0 = !paused.0;
        }
        return;
    }
    if paused.0 || !binds.just(&keys, Action::TargetNext) {
        return;
    }
    let fighting = |s: Option<&SimUnit>| {
        s.and_then(|s| sim.unit(s.0))
            .is_some_and(|u| u.alive && u.hostile && u.in_combat)
    };
    // Mid-fight, Tab cycles only the enemies already fighting you, so it never pulls the next pack.
    let in_fight = units.iter().any(|(_, _, _, s)| fighting(s));
    let candidates: Vec<TargetCandidate> = units
        .iter()
        .filter(|(_, _, _, s)| !in_fight || fighting(*s))
        .map(|(e, tf, u, _)| TargetCandidate {
            id: e.to_bits(),
            pos: to_core(tf.translation),
            hostile: u.hostile,
            alive: u.alive,
        })
        .collect();
    let current: Option<EntityId> = target.0.map(Entity::to_bits);
    let picked = tab_target(
        to_core(player.pos),
        player.yaw,
        current,
        &candidates,
        TAB_TARGET_RANGE,
    );
    if let Some(id) = picked {
        target.0 = Some(Entity::from_bits(id));
    }
}

/// Half extents of the pick box of a lootable corpse. It is a flat square wide enough for any
/// lying direction (the death pose is about 1.7 m long), because the standing sphere sat above
/// the body and missed the head and feet.
const LOOT_BOX_HALF: Vec3 = Vec3::new(1.4, 0.5, 1.4);
/// Same for a corpse with nothing to take (still targetable, e.g. a teammate to resurrect).
const CORPSE_BOX_HALF: Vec3 = Vec3::new(0.8, 0.4, 0.8);

/// What the picker needs to know about one unit.
pub struct Pickable<K> {
    pub key: K,
    pub pos: Vec3,
    pub unit: Unit,
}

/// Own ray picking: no render-backend dependency, so it behaves the same on native and
/// WebGL2. Living units are spheres, corpses lying boxes. Out of combat a lootable corpse under
/// the cursor wins over a nearer living unit (WoW-style), so a mob that wandered behind it
/// does not steal the click. `slack` widens every volume (finger taps).
pub fn pick_ray<K: Copy>(
    ray: (Vec3, Vec3),
    units: impl Iterator<Item = Pickable<K>>,
    colliders: &[Aabb],
    slack: f32,
) -> Option<K> {
    let (origin, dir) = ray;
    let units: Vec<_> = units.collect();
    let calm = !units.iter().any(|u| u.unit.fighting);
    let mut hits: Vec<(bool, f32, f32, K)> = units
        .iter()
        .filter_map(|u| {
            let (near, far) = if u.unit.alive {
                let center = u.pos + Vec3::Y * PICK_SPHERE_HEIGHT;
                let t = ray_sphere(origin, dir, center, PICK_SPHERE_RADIUS + slack)?;
                (t, t)
            } else {
                let half = if u.unit.lootable {
                    LOOT_BOX_HALF
                } else {
                    CORPSE_BOX_HALF
                };
                let half = half + Vec3::new(slack, 0.0, slack);
                let center = u.pos + Vec3::Y * half.y * 0.8;
                let t = ray_aabb(origin, dir, center - half, center + half)?;
                // Where the ray passes the body root: the fallback aim for the sight check.
                let closest = (u.pos - origin).dot(dir).clamp(t, t + 2.0 * half.length());
                (t, closest)
            };
            Some((calm && u.unit.lootable, near, far, u.key))
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)));
    let sees = |t: f32| line_of_sight(to_core(origin), to_core(origin + dir * t), colliders);
    hits.into_iter()
        .find(|(_, near, far, _)| sees(*near) || sees(*far))
        .map(|(.., k)| k)
}

pub fn pick_unit(
    cursor: Vec2,
    camera: (&Camera, &GlobalTransform),
    colliders: &[Aabb],
    units: &Query<(Entity, &Transform, &Unit)>,
    slack: f32,
) -> Option<Entity> {
    let (cam, cam_tf) = camera;
    let ray = cam.viewport_to_world(cam_tf, cursor).ok()?;
    pick_ray(
        (ray.origin, *ray.direction),
        units.iter().map(|(key, tf, u)| Pickable {
            key,
            pos: tf.translation,
            unit: *u,
        }),
        colliders,
        slack,
    )
}

fn click_select(
    mouse: Res<MouseState>,
    paused: Res<Paused>,
    colliders: Res<Colliders>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    units: Query<(Entity, &Transform, &Unit)>,
    ui: Query<&Interaction>,
    mut target: ResMut<Target>,
) {
    let Some(cursor) = mouse.click else { return };
    // A click on a HUD button must not also pick the unit behind it.
    if paused.0 || ui.iter().any(|i| *i != Interaction::None) {
        return;
    }
    if let Some(e) = pick_unit(cursor, *camera, &colliders.0, &units, 0.0) {
        target.0 = Some(e);
    }
}

/// Entry distance of the ray into the box, or `None` (slab method).
pub fn ray_aabb(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3) -> Option<f32> {
    let (mut near, mut far) = (0.0_f32, f32::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-6 {
            if origin[i] < lo[i] || origin[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - origin[i]) / dir[i], (hi[i] - origin[i]) / dir[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far).then_some(near)
}

pub fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = center - origin;
    let along = oc.dot(dir);
    let d2 = oc.length_squared() - along * along;
    let r2 = radius * radius;
    if along < 0.0 || d2 > r2 {
        return None;
    }
    Some(along - (r2 - d2).sqrt())
}

fn update_readout(
    sim: Option<Res<CombatSim>>,
    target: Res<Target>,
    colliders: Res<Colliders>,
    player: Single<&PlayerBody, With<Player>>,
    units: Query<(&Transform, &Unit)>,
    mut info: ResMut<TargetInfo>,
) {
    // Dev overlay only: range is taken from the first action-bar spell.
    let spell_range = sim
        .and_then(|s| {
            let id = *s.0.content().action_bar().first()?;
            s.0.content().spell(id).map(|d| d.range_m)
        })
        .unwrap_or(0.0);
    info.0 = target.0.and_then(|e| units.get(e).ok()).map(|(tf, u)| {
        let eye = player.pos + Vec3::Y * PLAYER_EYE;
        let chest = TargetCandidate {
            id: 0,
            pos: to_core(tf.translation + Vec3::Y * PICK_SPHERE_HEIGHT),
            hostile: u.hostile,
            alive: u.alive,
        };
        // Each chip isolates one rule by relaxing the other two, so the HUD
        // always agrees with what spell validation will say in Phase 2.
        let check = |range: f32, facing: bool, blockers: &[Aabb]| {
            validate_target(
                to_core(eye),
                player.yaw,
                Some(&chest),
                range,
                facing,
                blockers,
            )
        };
        let failed = |err: TargetError, result: Result<(), TargetError>| result == Err(err);
        TargetReadout {
            spell_range,
            distance: player.pos.distance(tf.translation),
            in_range: !failed(TargetError::OutOfRange, check(spell_range, false, &[])),
            facing: !failed(TargetError::NotFacing, check(f32::INFINITY, true, &[])),
            line_of_sight: !failed(
                TargetError::NoLineOfSight,
                check(f32::INFINITY, false, &colliders.0),
            ),
        }
    });
}

/// Ring candidates: every sim-linked body, the player's own (no `Unit`) included.
type RingUnits<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, Option<&'static Unit>),
    (With<SimUnit>, Without<TargetRing>),
>;

fn update_ring(
    target: Res<Target>,
    units: RingUnits,
    ring: Single<(
        &TargetRing,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let (data, mut tf, mut vis, mut mat) = ring.into_inner();
    let Some((unit_tf, unit)) = target.0.and_then(|e| units.get(e).ok()) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    // Intern is spawned with its centre at capsule height; the ring belongs on the floor.
    tf.translation = Vec3::new(unit_tf.translation.x, RING_HEIGHT, unit_tf.translation.z);
    mat.0 = if unit.is_some_and(|u| u.hostile) {
        data.hostile.clone()
    } else {
        data.friendly.clone()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(alive: bool, hostile: bool, lootable: bool, fighting: bool) -> Unit {
        Unit {
            hostile,
            alive,
            lootable,
            fighting,
        }
    }

    /// Camera 8 m up and 6 m back looking down at `at`, the usual follow-camera angle.
    fn ray_at(at: Vec3) -> (Vec3, Vec3) {
        let origin = at + Vec3::new(0.0, 8.0, 6.0);
        (origin, (at - origin).normalize())
    }

    fn pick(ray: (Vec3, Vec3), units: &[(u8, Vec3, Unit)], slack: f32) -> Option<u8> {
        pick_ray(
            ray,
            units
                .iter()
                .map(|&(key, pos, unit)| Pickable { key, pos, unit }),
            &[],
            slack,
        )
    }

    #[test]
    fn lootable_corpse_is_pickable_at_head_and_feet() {
        let corpse = [(1, Vec3::ZERO, unit(false, true, true, false))];
        // 1.2 m from the root along the floor: the old standing sphere missed this.
        for dx in [-1.2, 1.2] {
            assert_eq!(pick(ray_at(Vec3::new(dx, 0.1, 0.0)), &corpse, 0.0), Some(1));
        }
        assert_eq!(pick(ray_at(Vec3::new(2.4, 0.1, 0.0)), &corpse, 0.0), None);
        // A finger tap reaches a bit further.
        assert_eq!(
            pick(ray_at(Vec3::new(1.8, 0.1, 0.0)), &corpse, 0.5),
            Some(1)
        );
    }

    #[test]
    fn calm_click_prefers_the_lootable_corpse_over_a_nearer_live_unit() {
        let corpse = (1, Vec3::ZERO, unit(false, true, true, false));
        // Standing between the camera (+z) and the corpse, the intern is hit first.
        let intern = (2, Vec3::new(0.0, 0.0, 0.8), unit(true, false, false, false));
        let ray = ray_at(Vec3::new(0.0, 0.2, 0.3));
        assert_eq!(pick(ray, &[corpse, intern], 0.0), Some(1));
        // The same click mid-fight keeps the live unit, so targeting is not hijacked.
        let fighting = (3, Vec3::new(5.0, 0.0, 5.0), unit(true, true, false, true));
        assert_eq!(pick(ray, &[corpse, intern, fighting], 0.0), Some(2));
    }

    #[test]
    fn interact_loots_the_nearest_corpse_without_a_target() {
        let at = |x: f32| Vec3::new(x, 0.0, 0.0);
        let corpse = unit(false, true, true, false);
        let npc = unit(true, false, false, false);
        let units = [
            (1, at(4.0), corpse),
            (2, at(2.0), npc),
            (3, at(3.0), corpse),
        ];
        assert_eq!(
            crate::combat::interact_target(None, Vec3::ZERO, 6.0, &units),
            Some(3)
        );
        // Out of reach: nothing to loot, so the friendly in reach is talked to.
        assert_eq!(
            crate::combat::interact_target(None, Vec3::ZERO, 2.5, &units),
            Some(2)
        );
        assert_eq!(
            crate::combat::interact_target(None, Vec3::ZERO, 1.0, &units),
            None
        );
        // A live enemy target out of combat no longer blocks looting; in combat it does.
        let enemy = (9, at(8.0), unit(true, true, false, false));
        assert_eq!(
            crate::combat::interact_target(Some(9), Vec3::ZERO, 6.0, &[units[0], enemy]),
            Some(1)
        );
        let fighting = (9, at(8.0), unit(true, true, false, true));
        assert_eq!(
            crate::combat::interact_target(Some(9), Vec3::ZERO, 6.0, &[units[0], fighting]),
            Some(9)
        );
    }

    #[test]
    fn party_keys_map_to_self_then_other_members_in_join_order() {
        let players = [4, 2, 9];
        assert_eq!(party_slot(&players, 2, 0), Some(2));
        assert_eq!(party_slot(&players, 2, 1), Some(4));
        assert_eq!(party_slot(&players, 2, 2), Some(9));
        assert_eq!(party_slot(&players, 2, 3), None);
        // Offline: just you.
        assert_eq!(party_slot(&[7], 7, 1), None);
    }
}
