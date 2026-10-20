//! Accessories parented to skeleton bones so they follow every animation. The local player
//! carries a coffee mug in the left fist; every class skin (player, teammates, role picker,
//! lineup) gets its class kit: Fullstack headphones resting on the neck, Frontend glasses,
//! Backend big over-ear headphones, a DevOps pager on the belt, a QA clipboard on the other hip,
//! a Data Scientist's lanyard with a glowing chart badge. Offsets are in bone space
//! (model units; the model root is scaled).
//!
//! While the player channels Coffee Break the left arm is posed procedurally (two-bone IK
//! towards the mouth, blended over the animation) and the mug tilts for a sip every few seconds.

use crate::combat_anim::CombatAnim;
use crate::player::Player;
use bevy::app::AnimationSystems;
use bevy::light::NotShadowCaster;
use bevy::math::Affine3A;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

const NECK_BONE: &str = "Neck";
/// Class-kit offsets in bone space (model units, +Z forward), measured from the skin: the
/// head spans x +-0.54, y -0.1..1.0 (hair included), the face front sits at z 0.5 and the
/// eyes at y 0.3..0.6.
const GLASSES_Y: f32 = 0.47;
const GLASSES_Z: f32 = 0.54;
const CANS_Y: f32 = 0.42;
/// Hips-bone offset of the pager: clipped to the front of the belt (z 0.2), off to the side
/// where the hanging arm does not cover it.
const PAGER_AT: Vec3 = Vec3::new(0.27, 0.13, 0.27);
/// Hips-bone offset of the QA clipboard: on the hip opposite the pager, tilted like it hangs.
const CLIPBOARD_AT: Vec3 = Vec3::new(-0.3, 0.05, 0.25);
/// Neck-bone offset of the Data Scientist's badge: on the chest, below the collar.
const BADGE_AT: Vec3 = Vec3::new(0.0, -0.55, 0.36);
const HAND_BONE: &str = "Fist.L";
/// Yaw of the mug relative to the player, so the handle faces the camera side.
/// Mug origin in bone space before the lift.
const MUG_GRIP: Vec3 = Vec3::new(0.0, 0.12, 0.24);
const MUG_YAW: f32 = 3.0 * std::f32::consts::FRAC_PI_4;
/// Where the mug sits relative to the fist, in player space (metres): above it and a bit ahead.
const MUG_LIFT: Vec3 = Vec3::new(0.0, 0.15, -0.05);

/// Seconds to raise the arm into the drinking pose and to lower it again.
const DRINK_BLEND_IN: f32 = 0.3;
const DRINK_BLEND_OUT: f32 = 0.25;
/// One sip: the mug tips towards the face once per cycle.
const SIP_PERIOD: f32 = 1.5;
/// Mug tilt about the player's right axis: held ready, and at the lips.
const MUG_READY_TILT: f32 = 0.2;
const MUG_SIP_TILT: f32 = 1.0;
/// Mug centre relative to the head joint in player space (x right, y up, z back), in
/// model units: held ready below the chin, and raised to the lips during a sip.
const MUG_READY_POS: Vec3 = Vec3::new(-0.12, -0.12, -0.5);
const MUG_SIP_POS: Vec3 = Vec3::new(-0.03, 0.2, -0.34);
/// Where the mug centre sits relative to the wrist while drinking (player space, model units).
const MUG_IN_HAND: Vec3 = Vec3::new(0.0, 0.2, -0.04);
/// The elbow bends outwards (left) and down.
const ELBOW_POLE: Vec3 = Vec3::new(-0.7, -1.0, 0.0);

#[derive(Component)]
struct GearAttached;

/// The class kit (a `Content::classes` key) a character skin wears.
#[derive(Component, Clone, PartialEq, Eq)]
pub struct Accessories(pub String);

#[derive(Component)]
struct AccessoriesAttached;

pub struct GearPlugin;

impl Plugin for GearPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                regear,
                attach_gear,
                attach_accessories,
                update_drink,
                animate_steam,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            pose_arm_and_mug
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        );
    }
}

/// A freshly spawned model (first spawn or a role change) gets its gear again: the old
/// pieces went away with the old skeleton.
fn regear(
    mut commands: Commands,
    respawned: Query<Entity, Added<crate::anim::ModelSpawned>>,
    changed: Query<Entity, Changed<Accessories>>,
) {
    for e in respawned.iter().chain(changed.iter()) {
        commands
            .entity(e)
            .remove::<(GearAttached, AccessoriesAttached)>();
    }
}

/// The glTF scene spawns asynchronously, so keep looking until both bones exist.
fn attach_gear(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Single<Entity, (With<Player>, Without<GearAttached>)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let bone = |wanted: &str| {
        children
            .iter_descendants(*player)
            .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == wanted))
    };
    let (Some(hand), Some(upper), Some(lower), Some(head)) = (
        bone(HAND_BONE),
        bone("UpperArm.L"),
        bone("LowerArm.L"),
        bone("Head"),
    ) else {
        return;
    };
    commands.entity(*player).insert((
        GearAttached,
        DrinkRig {
            upper,
            lower,
            fist: hand,
            head,
        },
        DrinkState::default(),
    ));
    spawn_mug(&mut commands, &mut meshes, &mut materials, hand);
}

/// Class kit on the bones of any skin with `Accessories`, once the skeleton exists.
fn attach_accessories(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    wearers: Query<(Entity, &Accessories), Without<AccessoriesAttached>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (wearer, kit) in &wearers {
        let bone = |wanted: &str| {
            children
                .iter_descendants(wearer)
                .find(|e| names.get(*e).is_ok_and(|n| n.as_str() == wanted))
        };
        let (Some(neck), Some(head), Some(hips)) = (bone(NECK_BONE), bone("Head"), bone("Hips"))
        else {
            continue;
        };
        commands.entity(wearer).insert(AccessoriesAttached);
        let mut mat = |color: Color, rough: f32| {
            materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: rough,
                ..default()
            })
        };
        match kit.0.as_str() {
            "frontend" => {
                let frame = mat(Color::srgb(0.95, 0.2, 0.6), 0.4);
                let lens = materials.add(StandardMaterial {
                    base_color: Color::srgba(0.75, 0.9, 1.0, 0.35),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.1,
                    ..default()
                });
                let rim = meshes.add(Torus::new(0.11, 0.14));
                let glass = meshes.add(Cylinder::new(0.115, 0.01));
                let bridge = meshes.add(Cuboid::new(0.1, 0.03, 0.03));
                commands.entity(head).with_children(|c| {
                    for side in [-1.0f32, 1.0] {
                        let at = Transform::from_xyz(side * 0.18, GLASSES_Y, GLASSES_Z)
                            .with_rotation(Quat::from_rotation_x(FRAC_PI_2));
                        c.spawn((
                            Mesh3d(rim.clone()),
                            MeshMaterial3d(frame.clone()),
                            at,
                            NotShadowCaster,
                        ));
                        c.spawn((
                            Mesh3d(glass.clone()),
                            MeshMaterial3d(lens.clone()),
                            at,
                            NotShadowCaster,
                        ));
                    }
                    c.spawn((
                        Mesh3d(bridge),
                        MeshMaterial3d(frame.clone()),
                        Transform::from_xyz(0.0, GLASSES_Y + 0.02, GLASSES_Z),
                        NotShadowCaster,
                    ));
                });
            }
            "backend" => {
                // Big studio cans over the ears, band over the top of the head.
                let shell = mat(Color::srgb(0.08, 0.08, 0.1), 0.5);
                let accent = mat(Color::srgb(0.85, 0.15, 0.12), 0.5);
                let segment = meshes.add(Cuboid::new(0.24, 0.07, 0.14));
                let cup = meshes.add(Cylinder::new(0.24, 0.16));
                let ring = meshes.add(Torus::new(0.18, 0.23));
                commands.entity(head).with_children(|c| {
                    // The band is an arc of short segments over the hair, ear to ear.
                    for k in 0..=6 {
                        let a = -1.25 + 2.5 * k as f32 / 6.0;
                        let r = 0.66;
                        c.spawn((
                            Mesh3d(segment.clone()),
                            MeshMaterial3d(shell.clone()),
                            Transform::from_xyz(a.sin() * r, CANS_Y + a.cos() * r, -0.02)
                                .with_rotation(Quat::from_rotation_z(-a)),
                            NotShadowCaster,
                        ));
                    }
                    for side in [-1.0f32, 1.0] {
                        let at = Transform::from_xyz(side * 0.6, CANS_Y, -0.02)
                            .with_rotation(Quat::from_rotation_z(FRAC_PI_2));
                        c.spawn((
                            Mesh3d(cup.clone()),
                            MeshMaterial3d(shell.clone()),
                            at,
                            NotShadowCaster,
                        ));
                        c.spawn((
                            Mesh3d(ring.clone()),
                            MeshMaterial3d(accent.clone()),
                            at.with_translation(Vec3::new(side * 0.69, CANS_Y, -0.02)),
                            NotShadowCaster,
                        ));
                    }
                });
            }
            "devops" => {
                // Pager clipped to the belt, its screen glowing an unread alert.
                let case = mat(Color::srgb(0.12, 0.12, 0.14), 0.5);
                let screen = materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::rgb(0.4, 3.0, 0.6),
                    ..default()
                });
                let body = meshes.add(Cuboid::new(0.22, 0.28, 0.08));
                let lcd = meshes.add(Cuboid::new(0.16, 0.09, 0.01));
                commands.entity(hips).with_children(|c| {
                    c.spawn((
                        Mesh3d(body),
                        MeshMaterial3d(case),
                        Transform::from_translation(PAGER_AT),
                        NotShadowCaster,
                    ))
                    .with_child((
                        Mesh3d(lcd),
                        MeshMaterial3d(screen),
                        Transform::from_xyz(0.0, 0.05, 0.045),
                    ));
                });
            }
            "qa" => {
                // Clipboard of repro steps: brown board, white sheet, one red failed check.
                let board = mat(Color::srgb(0.45, 0.28, 0.12), 0.7);
                let paper = mat(Color::srgb(0.95, 0.95, 0.92), 0.8);
                let fail = materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::rgb(3.0, 0.2, 0.15),
                    ..default()
                });
                let back = meshes.add(Cuboid::new(0.26, 0.34, 0.03));
                let sheet = meshes.add(Cuboid::new(0.22, 0.28, 0.01));
                let mark = meshes.add(Cuboid::new(0.12, 0.03, 0.01));
                commands.entity(hips).with_children(|c| {
                    c.spawn((
                        Mesh3d(back),
                        MeshMaterial3d(board),
                        Transform::from_translation(CLIPBOARD_AT)
                            .with_rotation(Quat::from_rotation_z(0.2)),
                        NotShadowCaster,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Mesh3d(sheet),
                            MeshMaterial3d(paper),
                            Transform::from_xyz(0.0, -0.01, 0.02),
                        ));
                        b.spawn((
                            Mesh3d(mark),
                            MeshMaterial3d(fail),
                            Transform::from_xyz(0.0, 0.05, 0.03)
                                .with_rotation(Quat::from_rotation_z(0.6)),
                        ));
                    });
                });
            }
            "data_scientist" => {
                // Conference lanyard around the neck with a badge glowing a green chart.
                let strap = mat(Color::srgb(0.45, 0.2, 0.7), 0.6);
                let card = mat(Color::srgb(0.95, 0.95, 0.97), 0.6);
                let chart = materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::rgb(0.3, 2.6, 1.2),
                    ..default()
                });
                let loop_mesh = meshes.add(Torus::new(0.33, 0.36));
                let badge = meshes.add(Cuboid::new(0.18, 0.24, 0.02));
                let bar = meshes.add(Cuboid::new(0.03, 1.0, 0.01));
                commands.entity(neck).with_children(|c| {
                    c.spawn((
                        Mesh3d(loop_mesh),
                        MeshMaterial3d(strap),
                        Transform::from_xyz(0.0, -0.25, 0.08)
                            .with_rotation(Quat::from_rotation_x(1.1)),
                        NotShadowCaster,
                    ));
                    c.spawn((
                        Mesh3d(badge),
                        MeshMaterial3d(card),
                        Transform::from_translation(BADGE_AT),
                        NotShadowCaster,
                    ))
                    .with_children(|b| {
                        // A rising bar chart, three bars.
                        for (k, h) in [0.05f32, 0.09, 0.14].into_iter().enumerate() {
                            b.spawn((
                                Mesh3d(bar.clone()),
                                MeshMaterial3d(chart.clone()),
                                Transform::from_xyz(
                                    -0.05 + k as f32 * 0.05,
                                    -0.08 + h / 2.0,
                                    0.015,
                                )
                                .with_scale(Vec3::new(1.0, h, 1.0)),
                            ));
                        }
                    });
                });
            }
            _ => {
                let dark = mat(Color::srgb(0.32, 0.33, 0.37), 0.6);
                let cushion = mat(Color::srgb(0.1, 0.1, 0.12), 0.6);
                neck_headphones(&mut commands, &mut meshes, neck, dark, cushion);
            }
        }
    }
}

/// Fullstack: headphones resting on the neck, band behind, cups at the collarbones.
fn neck_headphones(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    neck: Entity,
    dark: Handle<StandardMaterial>,
    cushion: Handle<StandardMaterial>,
) {
    let band = meshes.add(Torus::new(0.3, 0.345));
    let cup = meshes.add(Cylinder::new(0.17, 0.1));
    let pad = meshes.add(Cylinder::new(0.13, 0.04));
    commands.entity(neck).with_children(|c| {
        c.spawn((
            Mesh3d(band),
            MeshMaterial3d(dark.clone()),
            Transform::from_xyz(0.0, -0.1, 0.0).with_rotation(Quat::from_rotation_x(0.35)),
            NotShadowCaster,
        ));
        for side in [-1.0, 1.0] {
            c.spawn((
                Mesh3d(cup.clone()),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(side * 0.33, -0.16, 0.12).with_rotation(Quat::from_rotation_z(
                    std::f32::consts::FRAC_PI_2 - side * 0.3,
                )),
                NotShadowCaster,
            ))
            .with_child((
                Mesh3d(pad.clone()),
                MeshMaterial3d(cushion.clone()),
                Transform::from_xyz(0.0, -side * 0.065, 0.0),
            ));
        }
    });
}

/// Keeps the mug upright in the world whatever the fist bone does.
#[derive(Component)]
struct MugHolder;

#[derive(Component)]
struct MugSteam(f32);

/// Mug local frame: +Y up, handle toward +X, origin at the body centre.
fn spawn_mug(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hand: Entity,
) {
    let mut lit = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.4,
            ..default()
        })
    };
    let navy = lit(Color::srgb(0.1, 0.17, 0.5));
    let stripe_mat = lit(Color::srgb(0.95, 0.75, 0.15));
    let logo_mat = lit(Color::srgb(0.95, 0.95, 0.98));
    let coffee = lit(Color::srgb(0.12, 0.06, 0.03));
    let rim_mat = lit(Color::srgb(0.85, 0.87, 0.95));
    let steam_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let body = meshes.add(ConicalFrustum {
        radius_top: 0.2,
        radius_bottom: 0.17,
        height: 0.42,
    });
    let band = meshes.add(ConicalFrustum {
        radius_top: 0.1895,
        radius_bottom: 0.1845,
        height: 0.06,
    });
    let disc = meshes.add(Cylinder::new(0.188, 0.01));
    let rim = meshes.add(Torus::new(0.19, 0.21));
    let handle = meshes.add(Torus::new(0.05, 0.11));
    let logo = meshes.add(Cuboid::new(0.07, 0.07, 0.02));
    let puff = meshes.add(Sphere::new(1.0));

    commands.entity(hand).with_children(|c| {
        c.spawn((
            MugHolder,
            Transform::from_translation(MUG_GRIP),
            Visibility::default(),
        ))
        .with_children(|m| {
            let mut put = |mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, t: Transform| {
                m.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    t,
                    NotShadowCaster,
                ));
            };
            put(&body, &navy, Transform::IDENTITY);
            put(&band, &stripe_mat, Transform::from_xyz(0.0, -0.06, 0.0));
            // Small logo plate on the side facing away from the handle.
            put(&logo, &logo_mat, Transform::from_xyz(0.0, 0.04, -0.19));
            put(&disc, &coffee, Transform::from_xyz(0.0, 0.175, 0.0));
            put(&rim, &rim_mat, Transform::from_xyz(0.0, 0.21, 0.0));
            // The torus ring lies in XZ; stand it up in XY so it reads as a side handle.
            put(
                &handle,
                &navy,
                Transform::from_xyz(0.25, 0.02, 0.0)
                    .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                    .with_scale(Vec3::new(0.9, 0.9, 1.3)),
            );
            for i in 0..6 {
                m.spawn((
                    MugSteam(i as f32 / 3.0),
                    Mesh3d(puff.clone()),
                    MeshMaterial3d(steam_mat.clone()),
                    Transform::from_xyz(0.0, 0.3, 0.0).with_scale(Vec3::ZERO),
                    NotShadowCaster,
                ));
            }
        });
    });
}

/// Left-arm bones the drinking pose drives.
#[derive(Component)]
struct DrinkRig {
    upper: Entity,
    lower: Entity,
    fist: Entity,
    head: Entity,
}

#[derive(Component, Default)]
struct DrinkState {
    /// 0 = animation only, 1 = fully in the drinking pose.
    blend: f32,
    /// Seconds into the current drink, drives the sip cycle.
    t: f32,
}

impl DrinkState {
    /// 0..1 bump once per `SIP_PERIOD`: rise, hold at the lips, lower.
    fn sip(&self) -> f32 {
        let p = (self.t / SIP_PERIOD).fract();
        smooth(0.2, 0.4, p) - smooth(0.65, 0.85, p)
    }
}

fn smooth(lo: f32, hi: f32, x: f32) -> f32 {
    let k = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn update_drink(time: Res<Time>, player: Single<(&CombatAnim, &mut DrinkState), With<Player>>) {
    let (anim, mut st) = player.into_inner();
    let dt = time.delta_secs();
    if anim.drinking && !anim.dead {
        st.blend = (st.blend + dt / DRINK_BLEND_IN).min(1.0);
        st.t += dt;
    } else {
        st.blend = (st.blend - dt / DRINK_BLEND_OUT).max(0.0);
        if st.blend == 0.0 {
            st.t = 0.0;
        }
    }
}

/// World transform from the bones' current local transforms: the propagated `GlobalTransform`
/// is one frame old here, because this runs between animation and propagation.
fn world_of(q: &Query<(&mut Transform, Option<&ChildOf>)>, e: Entity) -> Affine3A {
    let Ok((tf, parent)) = q.get(e) else {
        return Affine3A::IDENTITY;
    };
    let local = tf.compute_affine();
    match parent {
        Some(p) => world_of(q, p.parent()) * local,
        None => local,
    }
}

fn uniform_scale(a: &Affine3A) -> f32 {
    a.matrix3.x_axis.length()
}

/// Two-bone IK: elbow position for a shoulder `p0`, wrist target `t`, bone lengths and a pole hint.
fn elbow_for(p0: Vec3, t: Vec3, l1: f32, l2: f32, pole: Vec3) -> Vec3 {
    let to = t - p0;
    let d = to.length().clamp((l1 - l2).abs() + 1e-3, l1 + l2 - 1e-3);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    let side = (pole - dir * pole.dot(dir)).normalize_or(Vec3::NEG_Y);
    p0 + dir * a + side * h
}

/// Swings `rot` so the bone axis `from` points along `to`, keeping the roll as animated.
fn aim(rot: Quat, from: Vec3, to: Vec3) -> Quat {
    Quat::from_rotation_arc(from.normalize_or(Vec3::Y), to.normalize_or(Vec3::Y)) * rot
}

/// Runs after animation: raises the left arm to the mouth while drinking, and always keeps the
/// mug upright in the world (the fist bone tumbles with every animation), tipping it for sips.
fn pose_arm_and_mug(
    player: Single<(Entity, &DrinkRig, &DrinkState), With<Player>>,
    mut q: Query<(&mut Transform, Option<&ChildOf>)>,
    mugs: Query<(Entity, &ChildOf), With<MugHolder>>,
) {
    let (player_e, rig, st) = *player;
    let player_rot = world_of(&q, player_e).to_scale_rotation_translation().1;
    let blend = st.blend * st.blend * (3.0 - 2.0 * st.blend);
    let sip = st.sip();

    if blend > 0.001 {
        let (w_up, w_low, w_fist, w_head) = (
            world_of(&q, rig.upper),
            world_of(&q, rig.lower),
            world_of(&q, rig.fist),
            world_of(&q, rig.head),
        );
        let (p0, p1, p2) = (
            Vec3::from(w_up.translation),
            Vec3::from(w_low.translation),
            Vec3::from(w_fist.translation),
        );
        let (l1, l2) = (p0.distance(p1), p1.distance(p2));
        let s = uniform_scale(&w_head);
        let (_, up_rot, _) = w_up.to_scale_rotation_translation();
        let (_, low_rot, _) = w_low.to_scale_rotation_translation();

        let mug_at = MUG_READY_POS.lerp(MUG_SIP_POS, sip);
        let target = Vec3::from(w_head.translation) + player_rot * ((mug_at - MUG_IN_HAND) * s);
        let elbow = elbow_for(p0, target, l1, l2, player_rot * ELBOW_POLE);

        let (Ok(up_local), Ok(low_local)) = (
            q.get(rig.upper).map(|(t, _)| *t),
            q.get(rig.lower).map(|(t, _)| *t),
        ) else {
            return;
        };
        let parent_up = w_up * up_local.compute_affine().inverse();
        let parent_up_rot = parent_up.to_scale_rotation_translation().1;
        let new_up_world = aim(up_rot, p1 - p0, elbow - p0);
        let up_ik = parent_up_rot.inverse() * new_up_world;
        let up_rot_local = up_local.rotation.slerp(up_ik, blend);

        // Re-derive the lower arm from the blended upper arm, so a partial blend stays connected.
        let w_up2 = parent_up
            * Transform {
                rotation: up_rot_local,
                ..up_local
            }
            .compute_affine();
        let up2_rot = w_up2.to_scale_rotation_translation().1;
        let w_low2 = w_up2 * low_local.compute_affine();
        let elbow2 = Vec3::from(w_low2.translation);
        let (_, low2_rot, _) = w_low2.to_scale_rotation_translation();
        let wrist_dir = low_rot.inverse() * (p2 - p1);
        let cur_dir = low2_rot * wrist_dir;
        let new_low_world = aim(low2_rot, cur_dir, target - elbow2);
        let low_ik = up2_rot.inverse() * new_low_world;
        let low_rot_local = low_local.rotation.slerp(low_ik, blend);

        if let Ok((mut t, _)) = q.get_mut(rig.upper) {
            t.rotation = up_rot_local;
        }
        if let Ok((mut t, _)) = q.get_mut(rig.lower) {
            t.rotation = low_rot_local;
        }
    }

    // Mug: upright in the world as before; while drinking it rides the wrist and tips for sips.
    let w_fist = world_of(&q, rig.fist);
    let (_, fist_rot, _) = w_fist.to_scale_rotation_translation();
    let s = uniform_scale(&w_fist);
    let tilt = blend * (MUG_READY_TILT + (MUG_SIP_TILT - MUG_READY_TILT) * sip);
    let world_rot = player_rot * Quat::from_rotation_x(tilt) * Quat::from_rotation_y(MUG_YAW);
    let carried = w_fist.transform_point3(MUG_GRIP) + player_rot * MUG_LIFT;
    let sipping = Vec3::from(w_fist.translation) + player_rot * (MUG_IN_HAND * s);
    let pos = carried.lerp(sipping, blend);
    let inv = w_fist.inverse();
    for (mug, parent) in &mugs {
        if parent.parent() != rig.fist {
            continue;
        }
        if let Ok((mut tf, _)) = q.get_mut(mug) {
            tf.rotation = fist_rot.inverse() * world_rot;
            tf.translation = inv.transform_point3(pos);
        }
    }
}

/// Wisps rise from the coffee, and thicker and faster while sipping.
fn animate_steam(
    time: Res<Time>,
    drink: Single<&DrinkState, With<Player>>,
    mut puffs: Query<(&MugSteam, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let d = drink.blend;
    for (s, mut tf) in &mut puffs {
        let speed = 0.4 + 0.6 * d;
        let k = (t * speed + s.0).fract();
        // Puffs with phase >= 1 only exist while drinking; the others stop pausing then.
        let idle = f32::from(u8::from((t * 0.3 + s.0 * 2.0).sin() > -0.2));
        let active = if s.0 >= 1.0 {
            d
        } else {
            idle + (1.0 - idle) * d
        };
        let rise = (k * std::f32::consts::PI).sin();
        tf.translation = Vec3::new(
            0.04 * (t * 2.0 + s.0 * 6.0).sin(),
            0.26 + 0.45 * k,
            0.04 * (t * 1.7 + s.0 * 4.0).cos(),
        );
        tf.scale = Vec3::splat((0.045 + 0.02 * d) * rise * active);
    }
}
