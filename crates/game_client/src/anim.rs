//! Quaternius Ultimate Animated Character Pack: loading, animation graphs and
//! the clip state machine. All characters share one skinned rig and clip set.
//! Locomotion is derived from transform motion; combat clips come from the
//! `CombatAnim` counters on the same entity.

use crate::catalog::{IdlePose, Paint};
use crate::combat_anim::CombatAnim;
use crate::config::RUN_SPEED;
use crate::player::PlayerBody;
use bevy::animation::AnimationTargetId;
use bevy::gltf::{Gltf, GltfMaterialName};
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::time::Duration;

/// A character GLB, identified by its asset path (see `catalog::unit_visual`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelKind(pub &'static str);

impl ModelKind {
    pub const CEO: ModelKind = ModelKind("models/characters/Suit_Male_Bald.glb");
    pub const ENGINEER: ModelKind = ModelKind("models/characters/Engineer.glb");
    pub const INTERN: ModelKind = ModelKind("models/characters/Casual_Female.glb");

    fn path(self) -> &'static str {
        self.0
    }
}

/// Runtime material recolours for a unit (shirt, skin, hair ...), applied once
/// the model scene has spawned.
#[derive(Component, Clone, Copy)]
pub struct Paints(pub &'static [Paint]);

#[derive(Component)]
struct Painted;

/// Marks the spawned scene root under a unit, so the idle pose can tilt it.
#[derive(Component)]
struct ModelRoot;

#[derive(Resource, Default)]
struct PaintCache(HashMap<(AssetId<StandardMaterial>, usize, usize), Handle<StandardMaterial>>);

/// Put on a unit root; the model is spawned as a child once its glTF is loaded.
/// Requires `CombatAnim` so every animated entity can be driven by the combat bridge.
#[derive(Component)]
#[require(AnimState, CombatAnim, IdlePose)]
pub struct Character {
    pub kind: ModelKind,
    pub scale: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Clip {
    Idle,
    Walk,
    Run,
    Back,
    StrafeL,
    StrafeR,
    Air,
    Cast,
    Release,
    Attack,
    Death,
    /// Upper-body twin of `Release`, layered over locomotion while moving.
    ReleaseUpper,
    /// Upper-body hit reaction, layered over whatever the legs are doing.
    Hit,
    /// Seated idle: plays once and holds the last frame.
    Sit,
    /// Per-spell release poses (see `release_pose`); full-body one-shots.
    Victory,
    Roll,
    Hop,
    /// Per-spell cast loops (see `cast_clip`).
    CastInspect,
    CastHack,
    CastType,
}

struct Spec {
    clip: &'static str,
    /// Negative plays the clip reversed (Bevy wraps negative speeds on loops).
    speed: f32,
    /// Masked upper-body layer instead of a full-body base clip.
    overlay: bool,
}

const fn spec(clip: &'static str, speed: f32) -> Spec {
    Spec {
        clip,
        speed,
        overlay: false,
    }
}

const fn overlay(clip: &'static str, speed: f32) -> Spec {
    Spec {
        clip,
        speed,
        overlay: true,
    }
}

/// Indexed by `Clip as usize`.
const SPECS: [Spec; 20] = [
    spec("Idle", 1.0),
    spec("Walk", 1.0),
    spec("Run", 1.0),
    spec("Walk", -1.0),
    spec("Run", 1.0),
    spec("Run", 1.0),
    spec("Jump", 1.4),
    spec("Shoot_OneHanded", 0.5),
    spec("Shoot_OneHanded", 1.0),
    spec("SwordSlash", 1.4),
    spec("Death", 1.0),
    overlay("Shoot_OneHanded", 1.0),
    overlay("RecieveHit", 1.3),
    spec("SitDown", 1.0),
    spec("Victory", 1.0),
    spec("Roll", 1.0),
    spec("Jump", 1.4),
    spec("PickUp", 0.7),
    spec("SwordSlash", 0.45),
    spec("PickUp", 2.2),
];
const CLIP_COUNT: usize = SPECS.len();

impl Clip {
    fn looping(self) -> bool {
        !matches!(
            self,
            Clip::Air
                | Clip::Cast
                | Clip::Release
                | Clip::Attack
                | Clip::Death
                | Clip::ReleaseUpper
                | Clip::Hit
                | Clip::Sit
                | Clip::Victory
                | Clip::Roll
                | Clip::Hop
        )
    }
}

type Nodes = [AnimationNodeIndex; CLIP_COUNT];

/// Mask group holding the root and legs. Overlay clips mask it out, so they
/// only drive the torso, arms and head while locomotion keeps the legs.
const LOWER_BODY_GROUP: u32 = 1;
/// Overlay weight relative to locomotion (weight 1): 4 gives the overlay ~80% of the upper body.
const OVERLAY_WEIGHT: f32 = 4.0;
const OVERLAY_FADE_IN: f32 = 0.06;
const OVERLAY_FADE_OUT: f32 = 0.15;

struct ModelData {
    gltf: Handle<Gltf>,
    graph: Option<(Handle<AnimationGraph>, Nodes, [f32; CLIP_COUNT])>,
    built: bool,
    masked: bool,
}

fn is_upper_body(name: &str) -> bool {
    [
        "Abdomen", "Torso", "Neck", "Head", "Shoulder", "UpperArm", "LowerArm", "Fist",
    ]
    .iter()
    .any(|p| name.starts_with(p))
}

#[derive(Resource)]
struct Models(HashMap<ModelKind, ModelData>);

#[derive(Component)]
struct ModelSpawned;

/// Lives on the entity holding the `AnimationPlayer`.
#[derive(Component)]
struct AnimLink {
    owner: Entity,
    nodes: Nodes,
    durations: [f32; CLIP_COUNT],
}

#[derive(Component, Default)]
pub struct AnimState {
    prev: Option<Vec3>,
    vel: Vec3,
    current: Option<Clip>,
    one_shot: bool,
    seen_release: u32,
    seen_attack: u32,
    seen_hit: u32,
    /// Speed multiplier of the running one-shot (per-spell release pose); 0 = unset.
    mul: f32,
    /// Masked clip layered over locomotion (release while moving, hit reaction).
    overlay: Option<Clip>,
    moving: bool,
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaintCache>()
            .add_systems(Startup, load_models)
            .add_systems(
                Update,
                (spawn_models, attach_players, apply_paints, pose_lean, drive).chain(),
            );
    }
}

fn load_models(mut commands: Commands, assets: Res<AssetServer>) {
    // The three always-present models load now; catalog models load on first use.
    let map = [ModelKind::CEO, ModelKind::ENGINEER, ModelKind::INTERN]
        .into_iter()
        .map(|k| {
            (
                k,
                ModelData {
                    gltf: assets.load(k.path()),
                    graph: None,
                    built: false,
                    masked: false,
                },
            )
        })
        .collect();
    commands.insert_resource(Models(map));
}

fn build_graph(
    gltf: &Gltf,
    graphs: &mut Assets<AnimationGraph>,
    clip_assets: &Assets<AnimationClip>,
) -> Option<(Handle<AnimationGraph>, Nodes, [f32; CLIP_COUNT])> {
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut nodes = Vec::with_capacity(CLIP_COUNT);
    let mut durations = [1.0; CLIP_COUNT];
    for (i, s) in SPECS.iter().enumerate() {
        let handle = gltf.named_animations.get(s.clip)?.clone();
        durations[i] = clip_assets
            .get(&handle)
            .map_or(1.0, AnimationClip::duration);
        nodes.push(if s.overlay {
            graph.add_clip_with_mask(handle, 1 << LOWER_BODY_GROUP, 1.0, root)
        } else {
            graph.add_clip(handle, 1.0, root)
        });
    }
    Some((graphs.add(graph), nodes.try_into().ok()?, durations))
}

fn spawn_models(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    clip_assets: Res<Assets<AnimationClip>>,
    chars: Query<(Entity, &Character), Without<ModelSpawned>>,
) {
    for (entity, c) in &chars {
        let data = models.0.entry(c.kind).or_insert_with(|| ModelData {
            gltf: assets.load(c.kind.path()),
            graph: None,
            built: false,
            masked: false,
        });
        let Some(gltf) = gltfs.get(&data.gltf) else {
            continue; // still loading
        };
        if !data.built {
            data.built = true;
            data.graph = build_graph(gltf, &mut graphs, &clip_assets);
            if data.graph.is_none() {
                warn!(
                    "{}: missing animation clips, model stays static",
                    c.kind.path()
                );
            }
        }
        let Some(scene) = gltf.default_scene.clone() else {
            continue;
        };
        commands
            .entity(entity)
            .insert(ModelSpawned)
            .with_children(|p| {
                // The models face +Z; Bevy "forward" is -Z.
                p.spawn((
                    ModelRoot,
                    WorldAssetRoot(scene),
                    Transform::from_rotation(Quat::from_rotation_y(PI))
                        .with_scale(Vec3::splat(c.scale)),
                ));
            });
    }
}

fn find_owner(
    mut e: Entity,
    parents: &Query<&ChildOf>,
    chars: &Query<&Character>,
) -> Option<Entity> {
    while let Ok(p) = parents.get(e) {
        e = p.parent();
        if chars.contains(e) {
            return Some(e);
        }
    }
    None
}

// Bevy system params, not a call-site argument list.
#[allow(clippy::too_many_arguments)]
fn attach_players(
    mut commands: Commands,
    mut models: ResMut<Models>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    new: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    bones: Query<(&Name, &AnimationTargetId)>,
    chars: Query<&Character>,
) {
    for e in &new {
        let Some(owner) = find_owner(e, &parents, &chars) else {
            continue;
        };
        let Ok(c) = chars.get(owner) else { continue };
        let Some(data) = models.0.get_mut(&c.kind) else {
            continue;
        };
        let Some((graph, nodes, durations)) = data.graph.clone() else {
            continue;
        };
        // Target ids depend on the spawned hierarchy, so the mask group is
        // filled in once per model from its first instance. The player entity
        // itself may be the animated root node, hence the `once`.
        if !data.masked
            && let Some(mut g) = graphs.get_mut(&graph)
        {
            data.masked = true;
            for bone in std::iter::once(e).chain(children.iter_descendants(e)) {
                if let Ok((name, id)) = bones.get(bone)
                    && !is_upper_body(name.as_str())
                {
                    g.add_target_to_mask_group(*id, LOWER_BODY_GROUP);
                }
            }
        }
        commands.entity(e).insert((
            AnimationGraphHandle(graph),
            AnimationTransitions::new(),
            AnimLink {
                owner,
                nodes,
                durations,
            },
        ));
    }
}

/// Anything faster than this in one frame is a teleport (respawn, evade), not running.
const TELEPORT_SPEED: f32 = 40.0;
const VEL_SMOOTHING: f32 = 10.0;

fn drive(
    time: Res<Time>,
    mut owners: Query<(
        &Transform,
        &CombatAnim,
        &mut AnimState,
        Option<&PlayerBody>,
        &IdlePose,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions, &AnimLink)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut player, mut tr, link) in &mut players {
        let Ok((tf, ca, mut st, body, pose)) = owners.get_mut(link.owner) else {
            continue;
        };

        let pos = tf.translation;
        let raw = st.prev.map_or(Vec3::ZERO, |p| (pos - p) / dt);
        st.prev = Some(pos);
        let raw = Vec3::new(raw.x, 0.0, raw.z);
        let raw = if raw.length() > TELEPORT_SPEED {
            Vec3::ZERO
        } else {
            raw
        };
        let k = 1.0 - (-VEL_SMOOTHING * dt).exp();
        st.vel = st.vel.lerp(raw, k);

        // One-shot triggers.
        let mut trigger = None;
        let mut overlay_start = None;
        if ca.cast_release_seq != st.seen_release {
            st.seen_release = ca.cast_release_seq;
            if st.moving && !ca.dead {
                overlay_start = Some(Clip::ReleaseUpper);
            } else {
                let (clip, mul) = release_pose(ca.release_spell);
                trigger = Some(clip);
                st.mul = mul;
            }
        }
        if ca.attack_seq != st.seen_attack {
            st.seen_attack = ca.attack_seq;
            trigger = Some(Clip::Attack);
            st.mul = 1.0;
        }
        // Not restarted while it still plays: damage-over-time ticks would stutter it.
        if ca.hit_seq != st.seen_hit {
            st.seen_hit = ca.hit_seq;
            if !ca.dead && st.overlay != Some(Clip::Hit) {
                overlay_start = Some(Clip::Hit);
            }
        }

        if let Some(o) = overlay_start {
            if let Some(old) = st.overlay {
                player.stop(link.nodes[old as usize]);
            }
            player
                .start(link.nodes[o as usize])
                .set_weight(0.0)
                .set_speed(SPECS[o as usize].speed);
            st.overlay = Some(o);
        }
        if let Some(o) = st.overlay {
            let node = link.nodes[o as usize];
            match player.animation_mut(node) {
                Some(a) if !a.is_finished() && !ca.dead => {
                    let left = link.durations[o as usize] - a.seek_time();
                    let env = (a.elapsed() / OVERLAY_FADE_IN)
                        .min(left / OVERLAY_FADE_OUT)
                        .clamp(0.0, 1.0);
                    a.set_weight(OVERLAY_WEIGHT * env);
                }
                _ => {
                    player.stop(node);
                    st.overlay = None;
                }
            }
        }

        if st.one_shot
            && let Some(cur) = st.current
            && player
                .animation(link.nodes[cur as usize])
                .is_none_or(|a| a.is_finished())
        {
            st.one_shot = false;
        }

        let mut force = false;
        let want = if ca.dead {
            st.one_shot = false;
            Clip::Death
        } else if let Some(t) = trigger {
            st.one_shot = true;
            force = true;
            t
        } else if let (true, Some(cur)) = (st.one_shot, st.current) {
            cur
        } else {
            base_clip(&mut st, tf, ca, body, *pose)
        };

        if force || st.current != Some(want) {
            let fade = if st.current.is_none() { 0 } else { 150 };
            let anim = tr.play(
                &mut player,
                link.nodes[want as usize],
                Duration::from_millis(fade),
            );
            let mul = if force && st.mul > 0.0 { st.mul } else { 1.0 };
            anim.set_speed(SPECS[want as usize].speed * mul);
            if want.looping() {
                anim.repeat();
            }
            st.current = Some(want);
        }
    }
}

/// Cast loop per player spell id: Code Review inspects, Refactor hacks away, Write It By Hand types.
fn cast_clip(spell: u32) -> Clip {
    match spell {
        1 => Clip::CastInspect,
        2 => Clip::CastHack,
        11 => Clip::CastType,
        _ => Clip::Cast,
    }
}

/// Release pose and speed multiplier per player spell id (0 = Nitpick auto attack).
fn release_pose(spell: u32) -> (Clip, f32) {
    match spell {
        0 => (Clip::Release, 1.8),
        2 => (Clip::Attack, 1.0),
        3 => (Clip::Victory, 1.3),
        4 => (Clip::Release, 2.2),
        6 => (Clip::Release, 1.4),
        7 => (Clip::Roll, 1.5),
        8 => (Clip::Hop, 1.3),
        9 => (Clip::Attack, 0.9),
        10 => (Clip::Attack, 0.7),
        11 => (Clip::Victory, 1.0),
        _ => (Clip::Release, 1.0),
    }
}

fn base_clip(
    st: &mut AnimState,
    tf: &Transform,
    ca: &CombatAnim,
    body: Option<&PlayerBody>,
    pose: IdlePose,
) -> Clip {
    if matches!(pose, IdlePose::Sitting | IdlePose::Sleeping) && !ca.casting {
        return Clip::Sit;
    }
    if body.is_some_and(PlayerBody::airborne) {
        return Clip::Air;
    }
    let speed = st.vel.length();
    // Hysteresis: easier to keep moving than to start, so noisy speed doesn't flicker.
    st.moving = speed > if st.moving { 0.3 } else { 0.7 };
    if ca.casting {
        return cast_clip(ca.cast_spell);
    }
    if !st.moving {
        return Clip::Idle;
    }
    let local = tf.rotation.inverse() * st.vel;
    let (fwd, side) = (-local.z, local.x);
    if fwd < -0.4 * speed {
        Clip::Back
    } else if side.abs() > 0.8 * speed {
        if side > 0.0 {
            Clip::StrafeR
        } else {
            Clip::StrafeL
        }
    } else if speed > RUN_SPEED * 0.65 {
        Clip::Run
    } else {
        Clip::Walk
    }
}

/// The player model's hair is split into `Hair_1`..`Hair_5` (thinning stages, see `hair.rs`);
/// a "Hair" paint still covers every part.
fn paint_matches(slot: &str, material: &str) -> bool {
    material == slot
        || (slot == "Hair"
            && material
                .strip_prefix("Hair_")
                .is_some_and(|n| n.parse::<u8>().is_ok()))
}

/// Applies a unit's `Paints` to its model's materials. Recoloured materials are
/// cached per (base material, paint list, slot) so units of one type share them.
#[allow(clippy::type_complexity)] // Bevy query filter tuple
fn apply_paints(
    mut commands: Commands,
    chars: Query<(Entity, &Paints), (With<ModelSpawned>, Without<Painted>)>,
    children: Query<&Children>,
    mut meshes: Query<(&GltfMaterialName, &mut MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<PaintCache>,
) {
    for (entity, paints) in &chars {
        let mut found = false;
        for d in children.iter_descendants(entity) {
            let Ok((name, mut handle)) = meshes.get_mut(d) else {
                continue;
            };
            found = true;
            let Some((idx, paint)) = paints
                .0
                .iter()
                .enumerate()
                .find(|(_, p)| paint_matches(p.slot, &name.0))
            else {
                continue;
            };
            let key = (handle.0.id(), paints.0.as_ptr() as usize, idx);
            let painted = if let Some(h) = cache.0.get(&key) {
                h.clone()
            } else {
                let Some(base) = materials.get(&handle.0) else {
                    continue;
                };
                let mut m = base.clone();
                m.base_color = Color::srgb(paint.rgb[0], paint.rgb[1], paint.rgb[2]);
                if paint.glow {
                    let lin = m.base_color.to_linear();
                    m.emissive = LinearRgba::rgb(lin.red * 3.0, lin.green * 3.0, lin.blue * 3.0);
                }
                let h = materials.add(m);
                cache.0.insert(key, h.clone());
                h
            };
            handle.0 = painted;
        }
        if found {
            commands.entity(entity).insert(Painted);
        }
    }
}

/// Slumps sleeping units forward; every other pose stands upright.
fn pose_lean(
    poses: Query<&IdlePose>,
    mut roots: Query<(&ChildOf, &mut Transform), With<ModelRoot>>,
) {
    for (parent, mut tf) in &mut roots {
        let Ok(pose) = poses.get(parent.parent()) else {
            continue;
        };
        let lean = if *pose == IdlePose::Sleeping {
            0.5
        } else {
            0.0
        };
        let rot = Quat::from_rotation_y(PI) * Quat::from_rotation_x(lean);
        if tf.rotation != rot {
            tf.rotation = rot;
        }
    }
}
