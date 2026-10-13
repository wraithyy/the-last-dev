//! Kenney Blocky Characters: loading, animation graphs and the clip state
//! machine. The models are not skinned: eight rigid nodes animated by node
//! transforms. Locomotion is derived from transform motion; combat clips come
//! from the `CombatAnim` counters on the same entity.

use crate::combat_anim::CombatAnim;
use crate::config::RUN_SPEED;
use crate::player::PlayerBody;
use bevy::animation::{AnimationTargetId, VariableCurve};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    Ceo,
    Engineer,
    Intern,
}

impl ModelKind {
    const ALL: [ModelKind; 3] = [ModelKind::Ceo, ModelKind::Engineer, ModelKind::Intern];

    fn path(self) -> &'static str {
        match self {
            ModelKind::Ceo => "models/characters/character-q.glb",
            ModelKind::Engineer => "models/characters/character-f.glb",
            ModelKind::Intern => "models/characters/character-e.glb",
        }
    }
}

/// Put on a unit root; the model is spawned as a child once its glTF is loaded.
/// Requires `CombatAnim` so every animated entity can be driven by the combat bridge.
#[derive(Component)]
#[require(AnimState, CombatAnim)]
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
    AttackRight,
    AttackLeft,
    Death,
    /// Upper-body twin of `Release`, layered over locomotion while moving.
    ReleaseUpper,
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

/// Indexed by `Clip as usize`. The shoot clips last 0.2 s, too short to read, hence the slow-down.
const SPECS: [Spec; 13] = [
    spec("idle", 1.0),
    spec("walk", 1.0),
    spec("sprint", 1.0),
    spec("walk", -1.0),
    spec("walk", 1.0),
    spec("walk", 1.0),
    // No jump clip: the sprint pose reads as a leap.
    spec("sprint", 1.0),
    spec("holding-both", 1.0),
    spec("holding-both-shoot", 0.6),
    spec("attack-melee-right", 1.0),
    spec("attack-melee-left", 1.0),
    spec("die", 1.0),
    Spec {
        clip: "holding-right-shoot",
        speed: 0.6,
        overlay: true,
    },
];
const CLIP_COUNT: usize = SPECS.len();

impl Clip {
    fn looping(self) -> bool {
        !matches!(
            self,
            Clip::Release | Clip::AttackRight | Clip::AttackLeft | Clip::Death | Clip::ReleaseUpper
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
    ["torso", "arm-", "head"]
        .iter()
        .any(|p| name.starts_with(p))
}

#[derive(Resource)]
struct Models(HashMap<ModelKind, ModelData>);

#[derive(Component)]
struct ModelSpawned;

/// The scene root child of a unit; the flinch tilts it, as the clips never touch it.
#[derive(Component)]
struct ModelRoot;

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
    /// Masked clip layered over locomotion (release while moving).
    overlay: Option<Clip>,
    moving: bool,
    /// Seconds of the hit flinch left.
    flinch: f32,
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_models).add_systems(
            Update,
            (spawn_models, attach_players, drive, flinch).chain(),
        );
    }
}

fn load_models(mut commands: Commands, assets: Res<AssetServer>) {
    let map = ModelKind::ALL
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

/// Most clips animate only some nodes (idle has no legs, walk no torso), and a
/// node without a curve keeps its last pose when the previous clip fades out.
/// Filling the gaps from the `static` rest pose makes every base clip drive all nodes.
fn with_rest_pose(clip: &AnimationClip, rest: &AnimationClip) -> AnimationClip {
    let mut out = clip.clone();
    for (target, curves) in rest.curves() {
        if out.curves_for_target(*target).is_none() {
            for c in curves {
                out.add_variable_curve_to_target(*target, VariableCurve(c.0.clone_value()));
            }
        }
    }
    out
}

fn build_graph(
    gltf: &Gltf,
    graphs: &mut Assets<AnimationGraph>,
    clip_assets: &mut Assets<AnimationClip>,
) -> Option<(Handle<AnimationGraph>, Nodes, [f32; CLIP_COUNT])> {
    let rest = clip_assets
        .get(gltf.named_animations.get("static")?)?
        .clone();
    let mut completed: HashMap<&str, Handle<AnimationClip>> = HashMap::new();
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut nodes = Vec::with_capacity(CLIP_COUNT);
    let mut durations = [1.0; CLIP_COUNT];
    for (i, s) in SPECS.iter().enumerate() {
        let orig = gltf.named_animations.get(s.clip)?;
        let overlay = s.overlay;
        let handle = if overlay {
            orig.clone()
        } else if let Some(h) = completed.get(s.clip) {
            h.clone()
        } else {
            let full = with_rest_pose(clip_assets.get(orig)?, &rest);
            let h = clip_assets.add(full);
            completed.insert(s.clip, h.clone());
            h
        };
        durations[i] = clip_assets
            .get(&handle)
            .map_or(1.0, AnimationClip::duration);
        nodes.push(if overlay {
            graph.add_clip_with_mask(handle, 1 << LOWER_BODY_GROUP, 1.0, root)
        } else {
            graph.add_clip(handle, 1.0, root)
        });
    }
    Some((graphs.add(graph), nodes.try_into().ok()?, durations))
}

fn spawn_models(
    mut commands: Commands,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut clip_assets: ResMut<Assets<AnimationClip>>,
    chars: Query<(Entity, &Character), Without<ModelSpawned>>,
) {
    for (entity, c) in &chars {
        let Some(data) = models.0.get_mut(&c.kind) else {
            continue;
        };
        let Some(gltf) = gltfs.get(&data.gltf) else {
            continue; // still loading
        };
        if !data.built {
            data.built = true;
            data.graph = build_graph(gltf, &mut graphs, &mut clip_assets);
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
                // Kenney models face +Z; Bevy "forward" is -Z.
                p.spawn((
                    WorldAssetRoot(scene),
                    Transform::from_rotation(Quat::from_rotation_y(PI))
                        .with_scale(Vec3::splat(c.scale)),
                    ModelRoot,
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
    mut owners: Query<(&Transform, &CombatAnim, &mut AnimState, Option<&PlayerBody>)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions, &AnimLink)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut player, mut tr, link) in &mut players {
        let Ok((tf, ca, mut st, body)) = owners.get_mut(link.owner) else {
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
                trigger = Some(Clip::Release);
            }
        }
        if ca.attack_seq != st.seen_attack {
            st.seen_attack = ca.attack_seq;
            trigger = Some(if ca.attack_seq.is_multiple_of(2) {
                Clip::AttackLeft
            } else {
                Clip::AttackRight
            });
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
            base_clip(&mut st, tf, ca, body)
        };

        if force || st.current != Some(want) {
            let fade = if st.current.is_none() { 0 } else { 150 };
            let anim = tr.play(
                &mut player,
                link.nodes[want as usize],
                Duration::from_millis(fade),
            );
            anim.set_speed(SPECS[want as usize].speed);
            if want.looping() {
                anim.repeat();
            }
            st.current = Some(want);
        }
    }
}

fn base_clip(
    st: &mut AnimState,
    tf: &Transform,
    ca: &CombatAnim,
    body: Option<&PlayerBody>,
) -> Clip {
    if body.is_some_and(PlayerBody::airborne) {
        return Clip::Air;
    }
    let speed = st.vel.length();
    // Hysteresis: easier to keep moving than to start, so noisy speed doesn't flicker.
    st.moving = speed > if st.moving { 0.3 } else { 0.7 };
    if ca.casting {
        return Clip::Cast;
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

const FLINCH_SECS: f32 = 0.22;
/// Backward lean at the peak, about the feet; the clips never animate the model root.
const FLINCH_TILT: f32 = 0.18;

/// No hit clip exists, so a hit leans the whole model back for a moment; it
/// layers over any clip and never interrupts locomotion or casting.
fn flinch(
    time: Res<Time>,
    mut owners: Query<(&CombatAnim, &mut AnimState)>,
    mut models: Query<(&ChildOf, &mut Transform), With<ModelRoot>>,
) {
    for (child_of, mut tf) in &mut models {
        let Ok((ca, mut st)) = owners.get_mut(child_of.parent()) else {
            continue;
        };
        if ca.hit_seq != st.seen_hit {
            st.seen_hit = ca.hit_seq;
            if !ca.dead {
                st.flinch = FLINCH_SECS;
            }
        }
        if ca.dead {
            st.flinch = 0.0;
        }
        let tilt = if st.flinch > 0.0 {
            st.flinch = (st.flinch - time.delta_secs()).max(0.0);
            -FLINCH_TILT * (PI * st.flinch / FLINCH_SECS).sin()
        } else {
            0.0
        };
        tf.rotation = Quat::from_rotation_y(PI) * Quat::from_rotation_x(tilt);
    }
}
