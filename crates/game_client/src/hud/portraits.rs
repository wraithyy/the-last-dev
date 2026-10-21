//! Unit-frame portraits: every look (unit key + hair stage) is rendered once into a small
//! texture by a throw-away camera aimed at a duplicate of the model on a far-away stage, then
//! cached. The duplicate has no `Unit`/`SimUnit`, so nameplates, the minimap, distance culling
//! and the sim never see it. After a capture the rig, light and camera are despawned, so the
//! steady-state cost is zero. One capture at a time, bounded cache.

use crate::anim::ModelSpawned;
use crate::catalog;
use crate::rigs::PrimRig;
use crate::units::{CLASS_SKINS, spawn_skin, spawn_visual};
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;

const SIZE: u32 = 128;
/// Far below the world and the role-picker stage (y -500), out of any gameplay camera's view.
const STAGE: Vec3 = Vec3::new(0.0, -900.0, 0.0);
/// Frames the rig gets to load its glTF scene, bones, accessories and first idle pose.
const WARMUP: u32 = 10;
/// Frames the camera stays alive: the extract/render of the first one is what fills the image.
const CAMERA_FRAMES: u32 = 2;
/// Give up (letter fallback) when the model never shows up, e.g. a failed asset load.
const TIMEOUT: u32 = 240;
const MAX_CACHED: usize = 48;
/// Aim point above the head bone and camera distance, both as shares of the unit height.
const HEAD_LIFT: f32 = 0.09;
const CAMERA_DIST: f32 = 0.55;
const STAGES: usize = 6;

type Slots = [Option<Handle<Image>>; STAGES];

#[derive(Resource, Default)]
pub struct Portraits {
    ready: HashMap<String, Slots>,
    /// Insertion order, oldest first, for eviction.
    order: VecDeque<(String, u8)>,
    queue: VecDeque<(String, u8)>,
    failed: Vec<(String, u8)>,
    job: Option<Job>,
}

struct Job {
    look: (String, u8),
    rig: Entity,
    light: Entity,
    camera: Option<Entity>,
    image: Handle<Image>,
    frames: u32,
    warm: u32,
    camera_frames: u32,
}

impl Portraits {
    /// The cached portrait, or `None` while it is queued/captured (or if capturing failed).
    pub fn request(&mut self, key: &str, hair: u8) -> Option<Handle<Image>> {
        let hair = hair.min(STAGES as u8 - 1);
        if let Some(h) = self
            .ready
            .get(key)
            .and_then(|s| s[usize::from(hair)].as_ref())
        {
            return Some(h.clone());
        }
        let same = |l: &(String, u8)| l.0 == key && l.1 == hair;
        let known = self.job.as_ref().is_some_and(|j| same(&j.look))
            || self.queue.iter().any(same)
            || self.failed.iter().any(same);
        if !known {
            self.queue.push_back((key.to_string(), hair));
        }
        None
    }

    fn store(&mut self, look: (String, u8), image: Handle<Image>) {
        self.ready.entry(look.0.clone()).or_default()[usize::from(look.1)] = Some(image);
        self.order.push_back(look);
        while self.order.len() > MAX_CACHED {
            if let Some((k, h)) = self.order.pop_front()
                && let Some(slots) = self.ready.get_mut(&k)
            {
                slots[usize::from(h)] = None;
                if slots.iter().all(Option::is_none) {
                    self.ready.remove(&k);
                }
            }
        }
    }
}

pub struct PortraitPlugin;

impl Plugin for PortraitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Portraits>().add_systems(
            Update,
            capture.run_if(|p: Res<Portraits>| p.job.is_some() || !p.queue.is_empty()),
        );
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params
fn capture(
    mut commands: Commands,
    mut portraits: ResMut<Portraits>,
    mut images: ResMut<Assets<Image>>,
    models: Query<(), With<ModelSpawned>>,
    prims: Query<(), With<PrimRig>>,
    cameras: Query<(), With<Camera>>,
    entities: &bevy::ecs::entity::Entities,
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
) {
    let portraits = &mut *portraits;
    if portraits.job.is_none() {
        let Some(look) = portraits.queue.pop_front() else {
            return;
        };
        let (key, hair) = (look.0.as_str(), look.1);
        let height = catalog::unit_visual(key).height.max(0.3);
        let rig = match CLASS_SKINS.iter().position(|(_, unit)| *unit == key) {
            Some(class) => spawn_skin(&mut commands, class, STAGE, PI, hair),
            None => spawn_visual(&mut commands, key, STAGE, PI, catalog::IdlePose::Standing),
        };
        // A point light only: a directional one has no range and would also light the world.
        let r = height * 1.6;
        let light = commands
            .spawn((
                PointLight {
                    intensity: 60_000.0 * r * r,
                    range: r * 4.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(STAGE + Vec3::new(-0.5 * r, 0.8 * height + 0.3 * r, r)),
            ))
            .id();
        let image = images.add(Image::new_target_texture(
            SIZE,
            SIZE,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        portraits.job = Some(Job {
            look,
            rig,
            light,
            camera: None,
            image,
            frames: 0,
            warm: 0,
            camera_frames: 0,
        });
        return;
    }
    let Some(mut job) = portraits.job.take() else {
        return;
    };
    job.frames += 1;
    let Some(camera) = job.camera else {
        let ready = models.contains(job.rig) || prims.contains(job.rig);
        if job.frames > TIMEOUT && !ready {
            warn!(
                "portrait {:?}: model never loaded, keeping the letter",
                job.look
            );
            commands.entity(job.rig).despawn();
            commands.entity(job.light).despawn();
            portraits.failed.push(job.look);
            return;
        }
        // Warm-up counts from the moment the scene root exists.
        job.warm = if ready { job.warm + 1 } else { 0 };
        if job.warm >= WARMUP {
            let height = catalog::unit_visual(&job.look.0).height.max(0.3);
            // The head bone sits at the neck joint; aim a little above it so the face, hair and
            // headphones fill the frame. Primitive rigs have no bones and use a fixed share.
            let head = children
                .iter_descendants(job.rig)
                .find_map(|e| bones.get(e).ok().filter(|(n, _)| n.as_str() == "Head"))
                .map_or(STAGE + Vec3::new(0.0, height * 0.88, 0.0), |(_, t)| {
                    Vec3::new(STAGE.x, t.translation().y + height * HEAD_LIFT, STAGE.z)
                });
            // Slightly from the left so the face is not flat-lit; transparent clear so only the
            // head shows on the HUD.
            job.camera = Some(
                commands
                    .spawn((
                        Camera3d::default(),
                        Camera {
                            order: -3,
                            clear_color: Color::NONE.into(),
                            ..default()
                        },
                        RenderTarget::Image(job.image.clone().into()),
                        Transform::from_translation(
                            head + Vec3::new(-0.06, 0.02, CAMERA_DIST) * height,
                        )
                        .looking_at(head, Vec3::Y),
                    ))
                    .id(),
            );
        }
        portraits.job = Some(job);
        return;
    };
    job.camera_frames += 1;
    if job.camera_frames < CAMERA_FRAMES {
        portraits.job = Some(job);
        return;
    }
    for e in [job.rig, job.light, camera] {
        commands.entity(e).despawn();
    }
    info!(
        "portrait {:?} captured in {} frames; entities {} (pending despawn: 3 + rig children), cameras {}, cached {}",
        job.look,
        job.frames,
        entities.count_spawned(),
        cameras.iter().count(),
        portraits.order.len() + 1
    );
    portraits.store(job.look, job.image);
}
