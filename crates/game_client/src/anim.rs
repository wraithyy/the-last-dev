//! KayKit character models: loading, animation graphs and the clip state
//! machine. Locomotion is derived from transform motion; combat clips come
//! from the `CombatAnim` counters on the same entity.

use crate::combat_anim::CombatAnim;
use crate::config::RUN_SPEED;
use crate::player::PlayerBody;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    Mage,
    Rogue,
    Barbarian,
}

impl ModelKind {
    const ALL: [ModelKind; 3] = [ModelKind::Mage, ModelKind::Rogue, ModelKind::Barbarian];

    fn path(self) -> &'static str {
        match self {
            ModelKind::Mage => "models/characters/Mage.glb",
            ModelKind::Rogue => "models/characters/Rogue_Hooded.glb",
            ModelKind::Barbarian => "models/characters/Barbarian.glb",
        }
    }

    /// The glb ships every prop of the class; keep only the ones we want to see.
    fn hidden(self) -> &'static [&'static str] {
        match self {
            ModelKind::Mage => &["Spellbook", "Spellbook_open", "1H_Wand"],
            ModelKind::Rogue => &["1H_Crossbow", "2H_Crossbow", "Throwable"],
            ModelKind::Barbarian => &[
                "1H_Axe",
                "1H_Axe_Offhand",
                "2H_Axe",
                "Barbarian_Round_Shield",
            ],
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
    /// Extra emissive added to every material of this instance (enemy read).
    pub tint: Option<LinearRgba>,
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
    Chop,
    Slice,
    Hit,
    Death,
}

const CLIP_NAMES: [&str; 13] = [
    "Idle",
    "Walking_A",
    "Running_A",
    "Walking_Backwards",
    "Running_Strafe_Left",
    "Running_Strafe_Right",
    "Jump_Idle",
    "Spellcasting",
    "Spellcast_Shoot",
    "1H_Melee_Attack_Chop",
    "1H_Melee_Attack_Slice_Diagonal",
    "Hit_A",
    "Death_A",
];

impl Clip {
    fn looping(self) -> bool {
        !matches!(
            self,
            Clip::Release | Clip::Chop | Clip::Slice | Clip::Hit | Clip::Death
        )
    }
}

type Nodes = [AnimationNodeIndex; CLIP_NAMES.len()];

struct ModelData {
    gltf: Handle<Gltf>,
    graph: Option<(Handle<AnimationGraph>, Nodes)>,
    built: bool,
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
    moving: bool,
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_models).add_systems(
            Update,
            (spawn_models, attach_players, decorate, drive).chain(),
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
                },
            )
        })
        .collect();
    commands.insert_resource(Models(map));
}

fn build_graph(
    gltf: &Gltf,
    graphs: &mut Assets<AnimationGraph>,
) -> Option<(Handle<AnimationGraph>, Nodes)> {
    let clips: Option<Vec<_>> = CLIP_NAMES
        .iter()
        .map(|n| gltf.named_animations.get(*n).cloned())
        .collect();
    let (graph, nodes) = AnimationGraph::from_clips(clips?);
    Some((graphs.add(graph), nodes.try_into().ok()?))
}

fn spawn_models(
    mut commands: Commands,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
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
            data.graph = build_graph(gltf, &mut graphs);
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
                // KayKit models face +Z; Bevy "forward" is -Z.
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

fn attach_players(
    mut commands: Commands,
    models: Res<Models>,
    new: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    chars: Query<&Character>,
) {
    for e in &new {
        let Some(owner) = find_owner(e, &parents, &chars) else {
            continue;
        };
        let Ok(c) = chars.get(owner) else { continue };
        let Some((graph, nodes)) = models.0.get(&c.kind).and_then(|d| d.graph.clone()) else {
            continue;
        };
        commands.entity(e).insert((
            AnimationGraphHandle(graph),
            AnimationTransitions::new(),
            AnimLink { owner, nodes },
        ));
    }
}

/// Hides unwanted props and applies per-instance emissive tint to freshly spawned scene nodes.
fn decorate(
    named: Query<(Entity, &Name), Added<Name>>,
    mut meshes: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    parents: Query<&ChildOf>,
    chars: Query<&Character>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, name) in &named {
        if let Some(owner) = find_owner(e, &parents, &chars)
            && let Ok(c) = chars.get(owner)
            && c.kind.hidden().contains(&name.as_str())
        {
            commands.entity(e).insert(Visibility::Hidden);
        }
    }
    for (e, mut mat) in &mut meshes {
        let Some(owner) = find_owner(e, &parents, &chars) else {
            continue;
        };
        let Some(tint) = chars.get(owner).ok().and_then(|c| c.tint) else {
            continue;
        };
        if let Some(mut m) = materials.get(&mat.0).cloned() {
            m.emissive += tint;
            mat.0 = materials.add(m);
        }
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

        // One-shot triggers. Hit is the weakest: it never interrupts a cast or another one-shot.
        let mut trigger = None;
        if ca.cast_release_seq != st.seen_release {
            st.seen_release = ca.cast_release_seq;
            trigger = Some(Clip::Release);
        }
        if ca.attack_seq != st.seen_attack {
            st.seen_attack = ca.attack_seq;
            trigger = Some(if ca.attack_seq.is_multiple_of(2) {
                Clip::Slice
            } else {
                Clip::Chop
            });
        }
        if ca.hit_seq != st.seen_hit {
            st.seen_hit = ca.hit_seq;
            if trigger.is_none() && !ca.casting && !st.one_shot {
                trigger = Some(Clip::Hit);
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
