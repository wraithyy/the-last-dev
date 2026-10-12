//! The office arena, built from primitives. Every solid prop registers an
//! AABB so collision, camera clipping and line of sight share one source.

use crate::collision::aabb;
use crate::config::{ARENA_HALF, WALL_HEIGHT, WALL_THICKNESS};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use combat_core::Aabb;

#[derive(Component)]
pub struct Collider(pub Aabb);

/// Flat list of every static blocker, for combat_core line-of-sight queries.
#[derive(Resource, Default)]
pub struct Colliders(pub Vec<Aabb>);

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.75, 0.8, 1.0),
            brightness: 220.0,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.09)))
        .add_systems(Startup, build_arena)
        .add_systems(PostStartup, collect_colliders);
    }
}

struct Palette {
    floor: Handle<StandardMaterial>,
    wall: Handle<StandardMaterial>,
    partition: Handle<StandardMaterial>,
    desk: Handle<StandardMaterial>,
    chair: Handle<StandardMaterial>,
    monitor: Handle<StandardMaterial>,
    rack: Handle<StandardMaterial>,
    led_green: Handle<StandardMaterial>,
    led_red: Handle<StandardMaterial>,
    cable: Handle<StandardMaterial>,
}

fn matte(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    }
}

fn glow(color: LinearRgba) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    }
}

struct Builder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    meshes: &'a mut Assets<Mesh>,
}

impl Builder<'_, '_, '_> {
    fn boxed(
        &mut self,
        center: Vec3,
        size: Vec3,
        material: &Handle<StandardMaterial>,
        solid: bool,
    ) {
        let mut e = self.commands.spawn((
            Mesh3d(self.meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
        ));
        if solid {
            let bounds = aabb(center, size);
            e.insert(Collider(bounds));
        }
    }

    /// Open towards +Z: back and side partitions, desk with glowing monitor, chair.
    fn cubicle(&mut self, p: &Palette, x: f32, z: f32) {
        const SIZE: f32 = 3.0;
        const PART_H: f32 = 1.6;
        const PART_T: f32 = 0.1;
        let half = SIZE / 2.0;
        self.boxed(
            Vec3::new(x, PART_H / 2.0, z - half),
            Vec3::new(SIZE, PART_H, PART_T),
            &p.partition,
            true,
        );
        for side in [-1.0, 1.0] {
            self.boxed(
                Vec3::new(x + side * half, PART_H / 2.0, z),
                Vec3::new(PART_T, PART_H, SIZE),
                &p.partition,
                true,
            );
        }
        self.boxed(
            Vec3::new(x, 0.375, z - half + 0.5),
            Vec3::new(2.2, 0.75, 0.8),
            &p.desk,
            true,
        );
        self.boxed(
            Vec3::new(x, 1.0, z - half + 0.45),
            Vec3::new(0.7, 0.45, 0.05),
            &p.monitor,
            false,
        );
        self.boxed(
            Vec3::new(x, 0.3, z - half + 1.2),
            Vec3::new(0.5, 0.6, 0.5),
            &p.chair,
            false,
        );
    }

    fn server_rack(&mut self, p: &Palette, x: f32, z: f32) {
        self.boxed(
            Vec3::new(x, 1.1, z),
            Vec3::new(0.9, 2.2, 1.0),
            &p.rack,
            true,
        );
        for i in 0..6 {
            let y = 0.4 + i as f32 * 0.32;
            let led = if i % 3 == 1 { &p.led_red } else { &p.led_green };
            self.boxed(
                Vec3::new(x + 0.25, y, z + 0.51),
                Vec3::new(0.08, 0.08, 0.02),
                led,
                false,
            );
        }
    }
}

/// Derives the flat LoS list from the components so there is one source of truth.
fn collect_colliders(mut commands: Commands, q: Query<&Collider>) {
    commands.insert_resource(Colliders(q.iter().map(|c| c.0).collect()));
}

fn build_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let p = Palette {
        floor: materials.add(matte(Color::srgb(0.22, 0.24, 0.3))),
        wall: materials.add(matte(Color::srgb(0.55, 0.57, 0.65))),
        partition: materials.add(matte(Color::srgb(0.35, 0.42, 0.6))),
        desk: materials.add(matte(Color::srgb(0.6, 0.45, 0.3))),
        chair: materials.add(matte(Color::srgb(0.15, 0.15, 0.18))),
        monitor: materials.add(glow(LinearRgba::rgb(0.2, 2.2, 3.0))),
        rack: materials.add(matte(Color::srgb(0.1, 0.1, 0.12))),
        led_green: materials.add(glow(LinearRgba::rgb(0.2, 4.0, 0.6))),
        led_red: materials.add(glow(LinearRgba::rgb(4.0, 0.3, 0.2))),
        cable: materials.add(glow(LinearRgba::rgb(2.5, 0.4, 3.0))),
    };
    let mut b = Builder {
        commands: &mut commands,
        meshes: &mut meshes,
    };

    let span = ARENA_HALF * 2.0;
    b.boxed(
        Vec3::new(0.0, -0.1, 0.0),
        Vec3::new(span, 0.2, span),
        &p.floor,
        false,
    );

    // Walls sit outside the playable square so their inner face is at +-ARENA_HALF.
    let off = ARENA_HALF + WALL_THICKNESS / 2.0;
    let len = span + WALL_THICKNESS * 2.0;
    for s in [-1.0, 1.0] {
        b.boxed(
            Vec3::new(0.0, WALL_HEIGHT / 2.0, s * off),
            Vec3::new(len, WALL_HEIGHT, WALL_THICKNESS),
            &p.wall,
            true,
        );
        b.boxed(
            Vec3::new(s * off, WALL_HEIGHT / 2.0, 0.0),
            Vec3::new(WALL_THICKNESS, WALL_HEIGHT, len),
            &p.wall,
            true,
        );
    }

    // Two cubicle banks flank a clear central corridor (|x| < 12).
    for z in [-11.0, -4.0, 3.0] {
        for x in [-16.5, -13.5, 13.5, 16.5] {
            b.cubicle(&p, x, z);
        }
    }

    for x in [-3.0, 0.0, 3.0] {
        b.server_rack(&p, x, -ARENA_HALF + 0.6);
    }

    // Floor cables: decorative, not collidable.
    for x in [-3.0, 0.0, 3.0] {
        b.boxed(
            Vec3::new(x + 0.3, 0.02, -8.0),
            Vec3::new(0.1, 0.04, 22.0),
            &p.cable,
            false,
        );
    }
    b.boxed(
        Vec3::new(0.0, 0.02, 9.0),
        Vec3::new(20.0, 0.04, 0.1),
        &p.cable,
        false,
    );

    // One shadow-casting light only: WebGL2 budget.
    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1, // WebGL2 supports a single cascade
            maximum_distance: 60.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(8.0, 20.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (pos, color) in [
        (Vec3::new(-9.0, 3.4, -8.0), Color::srgb(0.4, 0.7, 1.0)),
        (Vec3::new(9.0, 3.4, -8.0), Color::srgb(0.4, 0.7, 1.0)),
        (Vec3::new(0.0, 3.0, -17.0), Color::srgb(0.8, 0.4, 1.0)),
        (Vec3::new(0.0, 3.4, 10.0), Color::srgb(1.0, 0.85, 0.6)),
    ] {
        commands.spawn((
            PointLight {
                color,
                intensity: 500_000.0,
                range: 22.0,
                ..default()
            },
            Transform::from_translation(pos),
        ));
    }
}
