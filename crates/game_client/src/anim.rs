//! Quaternius Ultimate Animated Character Pack: loading, animation graphs and
//! the clip state machine. All characters share one skinned rig and clip set.
//! Locomotion is derived from transform motion; combat clips come from the
//! `CombatAnim` counters on the same entity.

use crate::combat_anim::CombatAnim;
use crate::config::RUN_SPEED;
use crate::player::PlayerBody;
use bevy::animation::AnimationTargetId;
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
            ModelKind::Ceo => "models/characters/Suit_Male.glb",
            ModelKind::Engineer => "models/characters/Casual3_Male.glb",
            ModelKind::Intern => "models/characters/Casual_Female.glb",
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
    Attack,
    Death,
    /// Upper-body twin of `Release`, layered over locomotion while moving.
    ReleaseUpper,
    /// Upper-body hit reaction, layered over whatever the legs are doing.
    Hit,
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
const SPECS: [Spec; 13] = [
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
    /// Masked clip layered over locomotion (release while moving, hit reaction).
    overlay: Option<Clip>,
    moving: bool,
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_models)
            .add_systems(Update, (spawn_models, attach_players, drive).chain());
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
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    clip_assets: Res<Assets<AnimationClip>>,
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
            trigger = Some(Clip::Attack);
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
