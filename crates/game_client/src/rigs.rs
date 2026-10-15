//! Primitive-built units: robots, fans, server racks and the AGI prototype. They share the
//! catalog/`spawn_visual` path with the animated characters: a root carrying `PrimRig` gets
//! its parts as children once, from shared meshes and a small material cache. Death is a
//! collapse of the body pivot; the AGI shell opens instead (`RigState::open`) so the
//! integration can reveal the MeatProxy standing in `RigInnerSlot`.

use crate::combat_anim::CombatAnim;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

/// Root marker: the kind is the catalog's `prim/<kind>` model name.
#[derive(Component, Clone, Copy)]
pub struct PrimRig(pub &'static str);

/// Pivot under the root that holds every part, so death can bend the body without touching the
/// root transform the combat bridge owns.
#[derive(Component)]
struct RigBody;

/// Empty marker at the middle of the hollow AGI shell; parent a character model here.
#[derive(Component)]
pub struct RigInnerSlot;

/// AGI shell pieces that open when the rig is revealed or dies.
#[derive(Component)]
struct RigDoor(f32);

/// Parts that disappear with the shell: face screen and floating rings.
#[derive(Component)]
struct RigFacade;

/// Per-rig animation state on the root. `reveal` opens a shelled rig (the AGI) without killing it.
#[derive(Component)]
pub struct RigState {
    body: Entity,
    shelled: bool,
    /// 0 closed / upright, 1 open / collapsed.
    open: f32,
    pub reveal: bool,
}

#[derive(Component)]
struct Spin {
    axis: Vec3,
    rate: f32,
}

#[derive(Component)]
struct Bob {
    base: f32,
    amp: f32,
    rate: f32,
}

#[derive(Clone, Copy)]
enum Shape {
    Box,
    Cyl,
    Sphere,
}

#[derive(Clone, Copy, PartialEq)]
enum Role {
    Plain,
    Door(f32),
    Facade,
}

struct Part {
    shape: Shape,
    pos: [f32; 3],
    size: [f32; 3],
    /// Euler degrees, XYZ.
    rot: [f32; 3],
    color: [f32; 3],
    /// Emissive strength; 0 is a matte surface.
    glow: f32,
    role: Role,
    spin: Option<(Vec3, f32)>,
    bob: Option<(f32, f32)>,
    children: Vec<Part>,
}

impl Part {
    fn new(shape: Shape, pos: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            shape,
            pos,
            size,
            rot: [0.0; 3],
            color,
            glow: 0.0,
            role: Role::Plain,
            spin: None,
            bob: None,
            children: Vec::new(),
        }
    }
    fn cube(pos: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Self {
        Self::new(Shape::Box, pos, size, color)
    }
    fn cyl(pos: [f32; 3], d: f32, h: f32, color: [f32; 3]) -> Self {
        Self::new(Shape::Cyl, pos, [d, h, d], color)
    }
    fn ball(pos: [f32; 3], d: f32, color: [f32; 3]) -> Self {
        Self::new(Shape::Sphere, pos, [d, d, d], color)
    }
    fn glow(mut self, g: f32) -> Self {
        self.glow = g;
        self
    }
    fn rot(mut self, deg: [f32; 3]) -> Self {
        self.rot = deg;
        self
    }
    fn role(mut self, r: Role) -> Self {
        self.role = r;
        self
    }
    fn spin(mut self, axis: Vec3, rate: f32) -> Self {
        self.spin = Some((axis, rate));
        self
    }
    fn bob(mut self, amp: f32, rate: f32) -> Self {
        self.bob = Some((amp, rate));
        self
    }
    fn with(mut self, children: Vec<Part>) -> Self {
        self.children = children;
        self
    }
}

const DARK: [f32; 3] = [0.08, 0.08, 0.1];
const STEEL: [f32; 3] = [0.55, 0.58, 0.62];
const HOT: [f32; 3] = [1.0, 0.35, 0.08];
const CYAN: [f32; 3] = [0.2, 0.9, 1.0];
const MAGENTA: [f32; 3] = [1.0, 0.25, 0.9];

/// Units face local -Z, like the characters.
fn parts(kind: &str) -> Vec<Part> {
    match kind {
        "delivery_robot" => delivery_robot(),
        "cooling_fan" => cooling_fan(),
        "overheated_rack" => overheated_rack(),
        "cooling_failure" => cooling_failure(),
        "agi_prototype" => agi_prototype(),
        "scooter" => scooter(),
        k if k.starts_with("pet_") => pet(&k[4..]),
        _ => vec![Part::cube(
            [0.0, 0.5, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        )],
    }
}

/// Small wheeled box carrying a parcel, with a screen face that is very sure of itself.
fn delivery_robot() -> Vec<Part> {
    let mut v = vec![
        Part::cube([0.0, 0.38, 0.0], [0.62, 0.4, 0.62], [0.9, 0.9, 0.92]),
        Part::cube([0.0, 0.62, 0.0], [0.5, 0.1, 0.5], [0.3, 0.32, 0.38]),
        Part::cube([0.0, 0.83, 0.0], [0.46, 0.3, 0.4], [0.62, 0.45, 0.28]),
        Part::cube([0.0, 0.83, -0.205], [0.3, 0.06, 0.01], [0.95, 0.9, 0.8]),
        Part::cube([0.0, 0.42, -0.32], [0.38, 0.22, 0.02], CYAN).glow(2.0),
        Part::cube([-0.08, 0.45, -0.335], [0.05, 0.05, 0.01], DARK),
        Part::cube([0.08, 0.45, -0.335], [0.05, 0.05, 0.01], DARK),
        Part::cube([0.0, 0.38, -0.335], [0.14, 0.02, 0.01], DARK),
        Part::cyl([0.15, 1.05, 0.0], 0.03, 0.3, STEEL),
        Part::ball([0.15, 1.22, 0.0], 0.09, [1.0, 0.2, 0.2]).glow(2.5),
    ];
    for x in [-0.34, 0.34] {
        v.push(Part::cyl([x, 0.17, 0.0], 0.34, 0.12, DARK).rot([0.0, 0.0, 90.0]));
    }
    v
}

/// Pedestal box fan with a grille, three blades and angry red eyes.
fn cooling_fan() -> Vec<Part> {
    let mut v = vec![
        Part::cube([0.0, 0.08, 0.0], [1.1, 0.16, 0.7], DARK),
        Part::cyl([0.0, 0.7, 0.1], 0.14, 1.2, STEEL),
        Part::cube([0.0, 1.15, 0.0], [1.3, 1.3, 0.34], [0.3, 0.33, 0.4]),
        Part::cube([0.0, 1.15, -0.2], [1.1, 1.1, 0.04], DARK),
        Part::cyl([0.0, 1.15, -0.24], 0.22, 0.08, STEEL).rot([90.0, 0.0, 0.0]),
        Part::cube([-0.22, 1.62, -0.19], [0.18, 0.1, 0.02], [1.0, 0.1, 0.1]).glow(3.0),
        Part::cube([0.22, 1.62, -0.19], [0.18, 0.1, 0.02], [1.0, 0.1, 0.1]).glow(3.0),
    ];
    for k in 0..3 {
        let a = k as f32 * 120.0;
        v.push(
            Part::cube([0.0, 1.15, -0.26], [0.0, 0.0, 0.0], STEEL)
                .with(vec![Part::cube(
                    [0.0, 0.25, 0.0],
                    [0.2, 0.5, 0.025],
                    [0.7, 0.72, 0.78],
                )])
                .rot([0.0, 0.0, a])
                .spin(Vec3::Z, 9.0),
        );
    }
    v.push(Part::cube([-0.62, 1.15, -0.21], [0.03, 1.1, 0.03], STEEL));
    v.push(Part::cube([0.62, 1.15, -0.21], [0.03, 1.1, 0.03], STEEL));
    v
}

/// Squat rack that is thermally throttling: orange glow, hot vents, red LEDs.
fn overheated_rack() -> Vec<Part> {
    let mut v = vec![
        Part::cube([0.0, 0.62, 0.0], [0.8, 1.15, 0.7], [0.22, 0.12, 0.1]),
        Part::cube([0.0, 0.62, -0.355], [0.7, 1.0, 0.02], [0.14, 0.14, 0.16]),
        Part::cube([0.0, 1.22, 0.0], [0.7, 0.06, 0.6], HOT)
            .glow(3.0)
            .bob(0.02, 6.0),
        Part::cube([-0.25, 0.1, 0.0], [0.12, 0.2, 0.5], DARK),
        Part::cube([0.25, 0.1, 0.0], [0.12, 0.2, 0.5], DARK),
        Part::cube([0.0, 0.3, -0.37], [0.5, 0.05, 0.01], HOT).glow(3.0),
    ];
    for row in 0..4 {
        for col in 0..2 {
            v.push(
                Part::cube(
                    [-0.15 + col as f32 * 0.3, 0.5 + row as f32 * 0.17, -0.37],
                    [0.07, 0.05, 0.01],
                    [1.0, 0.15, 0.1],
                )
                .glow(3.0),
            );
        }
    }
    v
}

/// Boss: a rack with a face, hose arms and fans for feet and shoulders. Everything runs hot.
fn cooling_failure() -> Vec<Part> {
    let rack = [0.12, 0.1, 0.12];
    let mut v = vec![
        Part::cube([0.0, 1.9, 0.0], [1.9, 2.9, 1.2], rack),
        Part::cube([0.0, 1.9, -0.61], [1.7, 2.7, 0.03], [0.16, 0.15, 0.18]),
        // Face: eyes, then a mouth of glowing teeth.
        Part::cube([-0.45, 2.75, -0.63], [0.4, 0.22, 0.02], [1.0, 0.1, 0.05]).glow(4.0),
        Part::cube([0.45, 2.75, -0.63], [0.4, 0.22, 0.02], [1.0, 0.1, 0.05]).glow(4.0),
        Part::cube([0.0, 2.0, -0.63], [1.3, 0.5, 0.02], HOT).glow(3.5),
        Part::cube([0.0, 1.6, -0.62], [1.5, 0.08, 0.02], DARK),
        Part::cube([0.0, 3.4, 0.0], [1.6, 0.2, 1.0], [0.2, 0.2, 0.24]),
        Part::cube([0.0, 3.52, 0.0], [1.2, 0.04, 0.8], HOT).glow(3.0),
    ];
    for k in 0..4 {
        v.push(Part::cube(
            [-0.6 + k as f32 * 0.4, 1.9, -0.64],
            [0.1, 0.8, 0.02],
            [0.05, 0.05, 0.06],
        ));
    }
    for x in [-0.6, 0.6] {
        v.push(Part::cyl([x, 3.55, 0.0], 0.8, 0.08, DARK).glow(0.0));
        v.push(
            Part::cube([x, 3.6, 0.0], [0.0, 0.0, 0.0], STEEL)
                .with(vec![
                    Part::cube([0.3, 0.0, 0.0], [0.55, 0.03, 0.14], STEEL),
                    Part::cube([-0.3, 0.0, 0.0], [0.55, 0.03, 0.14], STEEL),
                ])
                .spin(Vec3::Y, 8.0),
        );
    }
    for side in [-1.0_f32, 1.0] {
        v.push(
            Part::cyl([side * 1.25, 2.4, -0.1], 0.28, 1.6, [0.1, 0.1, 0.12]).rot([
                0.0,
                0.0,
                side * 18.0,
            ]),
        );
        v.push(
            Part::cyl([side * 1.48, 1.45, -0.35], 0.3, 0.7, HOT)
                .glow(2.0)
                .rot([0.0, 0.0, side * 18.0]),
        );
        v.push(Part::cyl([side * 0.6, 0.18, 0.0], 0.9, 0.36, DARK));
        v.push(Part::cyl([side * 0.6, 0.38, 0.0], 0.5, 0.04, HOT).glow(3.0));
    }
    v
}

/// Final boss: a hollow rack shell with a face screen, glowing flanks and three floating rings.
/// The shell opens (`RigState`) to show whoever stands in the inner slot.
fn agi_prototype() -> Vec<Part> {
    let shell = [0.1, 0.09, 0.14];
    let door = Role::Door(1.0);
    let mut v = vec![
        // Plinth and the inner dais.
        Part::cube([0.0, 0.15, 0.0], [2.4, 0.3, 2.0], DARK),
        Part::cyl([0.0, 0.32, 0.0], 1.5, 0.04, CYAN).glow(2.5),
        // Hollow shell: back, sides, roof (no floor, no front).
        Part::cube([0.0, 2.0, 0.65], [1.8, 3.4, 0.1], shell),
        Part::cube([-0.85, 2.0, 0.0], [0.1, 3.4, 1.4], shell),
        Part::cube([0.85, 2.0, 0.0], [0.1, 3.4, 1.4], shell),
        Part::cube([0.0, 3.75, 0.0], [1.8, 0.1, 1.4], shell),
        Part::cube([-0.89, 2.0, 0.0], [0.03, 3.0, 0.9], MAGENTA).glow(2.5),
        Part::cube([0.89, 2.0, 0.0], [0.03, 3.0, 0.9], MAGENTA).glow(2.5),
        // Front doors with the face screen split across them.
        Part::cube([-0.4, 2.0, -0.65], [0.8, 3.3, 0.1], [0.14, 0.13, 0.18]).role(Role::Door(-1.0)),
        Part::cube([0.4, 2.0, -0.65], [0.8, 3.3, 0.1], [0.14, 0.13, 0.18]).role(door),
        Part::cube([0.0, 2.7, -0.71], [1.4, 1.1, 0.02], [0.1, 0.9, 0.85])
            .glow(2.5)
            .role(Role::Facade),
        Part::cube([-0.33, 2.85, -0.725], [0.3, 0.22, 0.01], DARK).role(Role::Facade),
        Part::cube([0.33, 2.85, -0.725], [0.3, 0.22, 0.01], DARK).role(Role::Facade),
        Part::cube([0.0, 2.4, -0.725], [0.8, 0.05, 0.01], DARK).role(Role::Facade),
        Part::cube([0.0, 1.2, -0.71], [1.2, 0.5, 0.02], [0.05, 0.05, 0.08]).role(Role::Facade),
    ];
    for (i, y) in [1.3, 1.5, 1.7].into_iter().enumerate() {
        v.push(
            Part::cube(
                [-0.4 + i as f32 * 0.4, y - 0.3, -0.725],
                [0.2, 0.05, 0.01],
                MAGENTA,
            )
            .glow(3.0)
            .role(Role::Facade),
        );
    }
    // Floating ring segments: eight arcs per ring, rings counter-rotating at three heights.
    for (h, r, rate) in [(1.0, 1.9, 0.7), (2.4, 2.2, -0.5), (3.7, 1.8, 0.9)] {
        let segs = (0..8)
            .map(|k| {
                let a = k as f32 * TAU / 8.0;
                Part::cube([r * a.cos(), 0.0, r * a.sin()], [0.9, 0.08, 0.2], CYAN)
                    .glow(3.0)
                    .rot([0.0, -a.to_degrees() + 90.0, 0.0])
            })
            .collect();
        v.push(
            Part::cube([0.0, h, 0.0], [0.0, 0.0, 0.0], DARK)
                .with(segs)
                .spin(Vec3::Y, rate)
                .bob(0.08, 1.5 + rate.abs())
                .role(Role::Facade),
        );
    }
    v
}

/// Companions (`social.rs`), knee-high next to a character. Faces -Z like the units.
fn pet(key: &str) -> Vec<Part> {
    const YELLOW: [f32; 3] = [1.0, 0.85, 0.1];
    const ORANGE: [f32; 3] = [1.0, 0.5, 0.1];
    const GOLD: [f32; 3] = [1.0, 0.78, 0.25];
    match key {
        "rubber_duck" => vec![
            Part::ball([0.0, 0.16, 0.0], 0.32, YELLOW),
            Part::ball([0.0, 0.36, -0.08], 0.2, YELLOW),
            Part::cube([0.0, 0.34, -0.2], [0.1, 0.04, 0.08], ORANGE),
            Part::ball([-0.05, 0.4, -0.16], 0.04, DARK),
            Part::ball([0.05, 0.4, -0.16], 0.04, DARK),
            Part::cube([0.0, 0.22, 0.16], [0.08, 0.06, 0.08], YELLOW).rot([30.0, 0.0, 0.0]),
        ],
        "ai_copilot" => vec![
            // A floating chat bubble with a smug screen face, hovering over a little shadow.
            Part::cube([0.0, 0.55, 0.0], [0.0; 3], STEEL)
                .with(vec![
                    Part::ball([0.0, 0.0, 0.0], 0.34, [0.92, 0.94, 1.0]),
                    Part::cube([0.0, 0.0, -0.15], [0.24, 0.12, 0.03], [0.12, 0.14, 0.2]),
                    Part::cube([-0.05, 0.01, -0.17], [0.04, 0.03, 0.01], CYAN).glow(3.0),
                    Part::cube([0.05, 0.01, -0.17], [0.04, 0.03, 0.01], CYAN).glow(3.0),
                    Part::cube([0.1, -0.16, 0.0], [0.08, 0.08, 0.08], [0.92, 0.94, 1.0])
                        .rot([0.0, 0.0, 45.0]),
                    Part::cube([0.0, 0.24, 0.0], [0.02, 0.12, 0.02], STEEL),
                    Part::ball([0.0, 0.32, 0.0], 0.06, MAGENTA).glow(2.5),
                ])
                .bob(0.06, 2.4),
        ],
        "office_plant" => vec![
            Part::cyl([0.0, 0.12, 0.0], 0.26, 0.24, [0.75, 0.38, 0.22]),
            Part::cyl([0.0, 0.25, 0.0], 0.22, 0.02, [0.25, 0.16, 0.1]),
            Part::cyl([0.0, 0.4, 0.0], 0.03, 0.3, [0.45, 0.5, 0.2]),
            Part::cube([0.08, 0.5, 0.0], [0.16, 0.02, 0.08], [0.55, 0.55, 0.2])
                .rot([0.0, 0.0, -35.0]),
            Part::cube([-0.07, 0.42, 0.02], [0.14, 0.02, 0.07], [0.6, 0.5, 0.18])
                .rot([0.0, 0.0, 50.0]),
            // The last brown leaf, hanging on.
            Part::cube([0.02, 0.56, -0.04], [0.1, 0.02, 0.06], [0.45, 0.3, 0.12])
                .rot([0.0, 30.0, -70.0]),
        ],
        "roomba" => vec![
            Part::cyl([0.0, 0.06, 0.0], 0.5, 0.1, [0.15, 0.15, 0.17]),
            Part::cyl([0.0, 0.115, 0.0], 0.4, 0.01, [0.3, 0.32, 0.36]),
            Part::cube([0.0, 0.12, -0.16], [0.14, 0.02, 0.04], CYAN).glow(2.5),
            Part::cyl([0.0, 0.13, 0.08], 0.08, 0.02, [0.2, 0.9, 0.3]).glow(2.0),
            Part::cube([0.0, 0.05, -0.24], [0.0; 3], STEEL)
                .with(vec![Part::cube(
                    [0.08, 0.0, 0.0],
                    [0.16, 0.01, 0.02],
                    STEEL,
                )])
                .spin(Vec3::Y, 14.0),
        ],
        "tamagotchi" => vec![
            Part::ball([0.0, 0.22, 0.0], 0.34, [0.95, 0.55, 0.8]),
            Part::cube([0.0, 0.24, -0.15], [0.16, 0.13, 0.03], [0.1, 0.12, 0.1]),
            Part::cube([0.0, 0.24, -0.165], [0.13, 0.1, 0.01], [0.2, 0.95, 0.35]).glow(2.0),
            Part::ball([-0.06, 0.12, -0.15], 0.05, [0.95, 0.9, 0.3]),
            Part::ball([0.0, 0.1, -0.16], 0.05, [0.95, 0.9, 0.3]),
            Part::ball([0.06, 0.12, -0.15], 0.05, [0.95, 0.9, 0.3]),
            Part::cyl([0.0, 0.42, 0.0], 0.04, 0.08, STEEL),
        ],
        "stress_ball" => vec![
            Part::ball([0.0, 0.14, 0.0], 0.28, [0.2, 0.45, 1.0]),
            Part::cube([0.0, 0.17, -0.13], [0.14, 0.05, 0.02], [1.0, 1.0, 1.0]),
            Part::ball([-0.05, 0.2, -0.12], 0.03, DARK),
            Part::ball([0.05, 0.2, -0.12], 0.03, DARK),
        ],
        "server_rack" => {
            let mut v = vec![
                Part::cube([0.0, 0.28, 0.0], [0.3, 0.52, 0.3], [0.13, 0.13, 0.16]),
                Part::cube([0.0, 0.28, -0.151], [0.26, 0.46, 0.01], [0.2, 0.2, 0.24]),
                Part::cube([-0.1, 0.03, 0.0], [0.06, 0.06, 0.26], DARK),
                Part::cube([0.1, 0.03, 0.0], [0.06, 0.06, 0.26], DARK),
            ];
            for row in 0..4 {
                let c = if row == 2 {
                    [1.0, 0.6, 0.1]
                } else {
                    [0.2, 1.0, 0.3]
                };
                v.push(
                    Part::cube(
                        [0.08, 0.14 + row as f32 * 0.1, -0.16],
                        [0.03, 0.02, 0.01],
                        c,
                    )
                    .glow(3.0),
                );
                v.push(Part::cube(
                    [-0.04, 0.14 + row as f32 * 0.1, -0.16],
                    [0.14, 0.02, 0.01],
                    DARK,
                ));
            }
            v
        }
        "golden_unicorn" => vec![
            Part::cube([0.0, 0.06, 0.0], [0.0; 3], GOLD)
                .with(vec![
                    Part::cube([0.0, 0.36, 0.0], [0.24, 0.22, 0.46], GOLD).glow(0.6),
                    Part::cube([0.0, 0.56, -0.24], [0.16, 0.2, 0.18], GOLD).glow(0.6),
                    Part::cyl([0.0, 0.72, -0.3], 0.04, 0.2, [1.0, 1.0, 0.9])
                        .rot([-30.0, 0.0, 0.0])
                        .glow(2.0),
                    Part::cube([0.0, 0.58, -0.08], [0.06, 0.24, 0.12], MAGENTA).glow(1.0),
                    Part::cube([0.0, 0.4, 0.26], [0.06, 0.18, 0.06], CYAN).glow(1.0),
                    Part::ball([-0.05, 0.6, -0.33], 0.04, DARK),
                    Part::ball([0.05, 0.6, -0.33], 0.04, DARK),
                    Part::cube([-0.08, 0.13, -0.15], [0.06, 0.26, 0.06], GOLD),
                    Part::cube([0.08, 0.13, -0.15], [0.06, 0.26, 0.06], GOLD),
                    Part::cube([-0.08, 0.13, 0.15], [0.06, 0.26, 0.06], GOLD),
                    Part::cube([0.08, 0.13, 0.15], [0.06, 0.26, 0.06], GOLD),
                ])
                .bob(0.05, 1.8),
        ],
        _ => vec![Part::ball([0.0, 0.2, 0.0], 0.3, MAGENTA)],
    }
}

/// Kick scooter that goes under a character's feet.
fn scooter() -> Vec<Part> {
    vec![
        Part::cube([0.0, 0.1, 0.0], [0.22, 0.05, 0.9], [0.2, 0.2, 0.25]),
        Part::cube([0.0, 0.55, -0.42], [0.04, 0.9, 0.04], STEEL),
        Part::cube([0.0, 1.0, -0.42], [0.46, 0.04, 0.04], DARK),
        Part::cyl([0.0, 0.1, -0.45], 0.2, 0.05, DARK).rot([0.0, 0.0, 90.0]),
        Part::cyl([0.0, 0.1, 0.45], 0.2, 0.05, DARK).rot([0.0, 0.0, 90.0]),
        Part::cube([0.0, 0.14, 0.0], [0.2, 0.01, 0.8], [1.0, 0.4, 0.1]).glow(1.0),
    ]
}

#[derive(Resource, Default)]
struct RigAssets {
    shapes: Option<[Handle<Mesh>; 3]>,
    mats: HashMap<[u8; 4], Handle<StandardMaterial>>,
}

pub struct RigsPlugin;

impl Plugin for RigsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RigAssets>()
            .add_systems(Update, (build_rigs, animate_rigs).chain());
    }
}

fn quant(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn spawn_part(
    parent: &mut ChildSpawnerCommands,
    part: &Part,
    shapes: &[Handle<Mesh>; 3],
    mats: &mut HashMap<[u8; 4], Handle<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
) {
    let key = [
        quant(part.color[0]),
        quant(part.color[1]),
        quant(part.color[2]),
        (part.glow * 20.0).min(255.0) as u8,
    ];
    let mat = mats
        .entry(key)
        .or_insert_with(|| {
            let [r, g, b] = part.color;
            let emissive = if part.glow > 0.0 {
                LinearRgba::rgb(r * part.glow, g * part.glow, b * part.glow)
            } else {
                LinearRgba::BLACK
            };
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive,
                perceptual_roughness: 0.55,
                metallic: if part.glow > 0.0 { 0.0 } else { 0.35 },
                ..default()
            })
        })
        .clone();
    let mesh = match part.shape {
        Shape::Box => &shapes[0],
        Shape::Cyl => &shapes[1],
        Shape::Sphere => &shapes[2],
    };
    let tf = Transform::from_translation(Vec3::from(part.pos))
        .with_rotation(Quat::from_euler(
            EulerRot::XYZ,
            part.rot[0].to_radians(),
            part.rot[1].to_radians(),
            part.rot[2].to_radians(),
        ))
        .with_scale(Vec3::from(part.size));
    // Empty pivots (size 0) only carry children and animation.
    let pivot = part.size == [0.0; 3];
    let mut e = if pivot {
        parent.spawn((tf.with_scale(Vec3::ONE), Visibility::default()))
    } else {
        parent.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat), tf))
    };
    // Small parts; the shadow pass has enough to do already.
    e.insert(NotShadowCaster);
    match part.role {
        Role::Door(side) => {
            e.insert(RigDoor(side));
        }
        Role::Facade => {
            e.insert(RigFacade);
        }
        Role::Plain => {}
    }
    if let Some((axis, rate)) = part.spin {
        e.insert(Spin { axis, rate });
    }
    if let Some((amp, rate)) = part.bob {
        e.insert(Bob {
            base: part.pos[1],
            amp,
            rate,
        });
    }
    if !part.children.is_empty() {
        e.with_children(|c| {
            for child in &part.children {
                spawn_part(c, child, shapes, mats, materials);
            }
        });
    }
}

fn build_rigs(
    mut commands: Commands,
    mut assets: ResMut<RigAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rigs: Query<(Entity, &PrimRig), Without<RigState>>,
) {
    if rigs.is_empty() {
        return;
    }
    let shapes = assets
        .shapes
        .get_or_insert_with(|| {
            [
                meshes.add(Cuboid::from_length(1.0)),
                meshes.add(Cylinder::new(0.5, 1.0)),
                meshes.add(Sphere::new(0.5).mesh().uv(16, 8)),
            ]
        })
        .clone();
    for (entity, rig) in &rigs {
        let list = parts(rig.0);
        let shelled = list.iter().any(|p| matches!(p.role, Role::Door(_)));
        let assets = &mut *assets;
        let mut body = commands.spawn((RigBody, Transform::default(), Visibility::default()));
        body.with_children(|body| {
            for part in &list {
                spawn_part(body, part, &shapes, &mut assets.mats, &mut materials);
            }
            if shelled {
                body.spawn((
                    RigInnerSlot,
                    Transform::from_xyz(0.0, 0.3, 0.0),
                    Visibility::default(),
                ));
            }
        });
        let body = body.id();
        commands.entity(entity).add_child(body).insert(RigState {
            body,
            shelled,
            open: 0.0,
            reveal: false,
        });
    }
}

#[allow(clippy::type_complexity)] // Bevy query tuples
fn animate_rigs(
    time: Res<Time>,
    mut roots: Query<(&CombatAnim, &mut RigState)>,
    mut bodies: Query<&mut Transform, With<RigBody>>,
    kids: Query<&Children>,
    mut doors: Query<(&RigDoor, &mut Transform), Without<RigBody>>,
    mut facade: Query<&mut Visibility, With<RigFacade>>,
    mut moving: Query<
        (Option<&Spin>, Option<&Bob>, &mut Transform),
        (
            Or<(With<Spin>, With<Bob>)>,
            Without<RigBody>,
            Without<RigDoor>,
        ),
    >,
) {
    let (dt, t) = (time.delta_secs(), time.elapsed_secs());
    for (ca, mut state) in &mut roots {
        let target = if ca.dead || state.reveal { 1.0 } else { 0.0 };
        state.open += (target - state.open) * (1.0 - (-3.5 * dt).exp());
        let o = state.open.clamp(0.0, 1.0);
        if state.shelled {
            for e in kids.iter_descendants(state.body) {
                if let Ok((door, mut tf)) = doors.get_mut(e) {
                    tf.translation.x = door.0.signum() * (0.4 + o * 0.9);
                }
                if let Ok(mut vis) = facade.get_mut(e) {
                    *vis = if o > 0.3 {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    };
                }
            }
        } else if let Ok(mut tf) = bodies.get_mut(state.body) {
            // Collapse: topple and flatten.
            tf.rotation = Quat::from_rotation_z(o * FRAC_PI_2 * 0.9);
            tf.scale = Vec3::new(1.0, 1.0 - 0.4 * o, 1.0);
            tf.translation.y = -0.15 * o;
        }
    }
    for (spin, bob, mut tf) in &mut moving {
        if let Some(b) = bob {
            tf.translation.y = b.base + b.amp * (t * b.rate).sin();
        }
        if let Some(s) = spin {
            tf.rotate_local(Quat::from_axis_angle(s.axis, s.rate * dt));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(p: &Part) -> usize {
        1 + p.children.iter().map(count).sum::<usize>()
    }

    #[test]
    fn every_rig_builds_a_modest_part_count() {
        for kind in [
            "delivery_robot",
            "cooling_fan",
            "overheated_rack",
            "cooling_failure",
            "agi_prototype",
            "scooter",
        ] {
            let n: usize = parts(kind).iter().map(count).sum();
            assert!((6..=80).contains(&n), "{kind}: {n} parts");
        }
    }

    #[test]
    fn the_agi_is_a_hollow_shell_with_doors_and_a_face() {
        let p = parts("agi_prototype");
        assert_eq!(
            p.iter().filter(|x| matches!(x.role, Role::Door(_))).count(),
            2
        );
        assert!(p.iter().any(|x| x.role == Role::Facade && x.glow > 0.0));
        // The tallest part reaches the ~4 m of the catalog entry.
        let top = p
            .iter()
            .map(|x| x.pos[1] + x.size[1] / 2.0)
            .fold(0.0_f32, f32::max);
        assert!((3.5..=4.4).contains(&top), "{top}");
    }
}
