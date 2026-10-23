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
            (mark_hatch, pulse)
                .chain()
                .after(FloorSwap)
                .in_set(SimReady),
        );
    }
}

#[derive(Component)]
struct HatchGlow(f32);

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
