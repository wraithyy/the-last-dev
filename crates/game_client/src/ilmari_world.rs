//! The post-credits world on the client (docs/ILMARI_09.md B.2): the glow over the hatch behind
//! the Car Park P3 attendant's booth that leads down to the Resistance base. The hatch itself is
//! plain steel in the P3 layout, so the car park gives nothing away before the Opening; once it
//! has played, this marks the way. Offline the local save decides, online the room's door; per zone load: the marker
//! is `ZoneDecor`, which the zone switch clears.

use crate::combat::{CombatSim, SimReady};
use crate::floors::carpark;
use crate::game::{FloorSwap, GameFlow, ZoneDecor};
use crate::net::NetWorld;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

pub struct IlmariWorldPlugin;

impl Plugin for IlmariWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (mark_hatch, pulse, place_label)
                .chain()
                .after(FloorSwap)
                .in_set(SimReady),
        );
    }
}

#[derive(Component)]
struct HatchGlow(f32);

/// The sign over the hatch: a UI label pinned to it, like a nameplate.
#[derive(Component)]
struct HatchLabel;

/// The sign shows this close to the hatch, so it reads from the row of cars but not the lot's end.
const LABEL_RANGE_M: f32 = 45.0;
const LABEL_HEIGHT_M: f32 = 4.0;

fn mark_hatch(
    mut commands: Commands,
    flow: Res<GameFlow>,
    sim: Res<CombatSim>,
    net: Option<Res<NetWorld>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut seen: Local<String>,
) {
    if *seen == flow.zone {
        return;
    }
    seen.clone_from(&flow.zone);
    // Online the room's door state is the truth: the hatch link opens with the room's reveal.
    let open = match net.as_deref() {
        Some(n) => crate::combat::SimRead::zone_links(n, carpark::ZONE)
            .iter()
            .any(|l| l.to == combat_core::ilmari::BASE_ZONE && l.unlocked),
        None => sim.0.ilmari_open(),
    };
    if flow.zone != carpark::ZONE || !open {
        return;
    }
    let (x, z) = carpark::HATCH;
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgba(0.3, 1.0, 0.55, 0.28),
        emissive: LinearRgba::rgb(0.6, 2.4, 1.1),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    // A ring on the floor and a thin beam above it: readable from the whole row of cars.
    let ring = meshes.add(Annulus::new(0.9, 1.15));
    commands.spawn((
        ZoneDecor,
        HatchLabel,
        crate::hud::label("DEFINITELY NOT A RESISTANCE BASE", 14.0),
        TextColor(Color::srgb(0.55, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(4),
    ));
    commands.spawn((
        ZoneDecor,
        HatchGlow(0.0),
        Mesh3d(ring),
        MeshMaterial3d(glow.clone()),
        Transform::from_xyz(x, 0.12, z)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.spawn((
        ZoneDecor,
        HatchGlow(1.7),
        Mesh3d(meshes.add(Cylinder::new(0.55, 6.0))),
        MeshMaterial3d(glow),
        Transform::from_xyz(x, 3.0, z),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn pulse(time: Res<Time>, mut glows: Query<(&HatchGlow, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (g, mut tf) in &mut glows {
        let k = 1.0 + 0.08 * (t * 2.4 + g.0).sin();
        tf.scale = Vec3::new(k, 1.0, k);
    }
}

fn place_label(
    hidden: Res<crate::hud::HudHidden>,
    paused: Res<crate::input::Paused>,
    ui_scale: Res<UiScale>,
    player: Query<&Transform, With<crate::player::Player>>,
    camera: Single<(&Camera, &GlobalTransform), With<crate::camera::OrbitCamera>>,
    mut label: Query<(&mut Node, &mut Visibility, &ComputedNode), With<HatchLabel>>,
) {
    let (x, z) = carpark::HATCH;
    let at = Vec3::new(x, LABEL_HEIGHT_M, z);
    let near = player.single().is_ok_and(|p| {
        Vec2::new(p.translation.x - x, p.translation.z - z).length() <= LABEL_RANGE_M
    });
    let (cam, cam_tf) = *camera;
    let on_screen = if near && !hidden.cutscene && !hidden.opening && !paused.0 {
        cam.world_to_viewport(cam_tf, at).ok()
    } else {
        None
    };
    let scale = ui_scale.0.max(0.01);
    for (mut node, mut vis, size) in &mut label {
        let Some(p) = on_screen else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        // Centred on the hatch; the first frame has no size yet and sits half a label off.
        let half = size.size().x * size.inverse_scale_factor() / 2.0;
        node.left = px(p.x / scale - half);
        node.top = px(p.y / scale);
        vis.set_if_neq(Visibility::Inherited);
    }
}
