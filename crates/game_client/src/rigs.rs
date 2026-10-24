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
    /// A flat panel wearing a drawn picture (`floors::rig_screen`), lit like a screen.
    Screen(&'static str),
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
    /// A panel that shows `picture` (see `floors::rig_screen`), full brightness.
    fn screen(pos: [f32; 3], size: [f32; 3], picture: &'static str) -> Self {
        Self::new(Shape::Box, pos, size, [1.0; 3]).role(Role::Screen(picture))
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
        "cutover_rack" => cutover_rack(),
        "tentacle_rack" => tentacle_rack(),
        "patch_tuesday" => patch_tuesday(),
        "perf_review" => perf_review(),
        "hr_bot" => hr_bot(),
        "scraper_queen" => scraper_queen(),
        "fan_elemental" => fan_elemental(),
        "generator_golem" => generator_golem(),
        "reserved_bot" => reserved_bot(),
        "rack_golem" => rack_golem(),
        "pager" => pager(),
        "autoscaler" => autoscaler(),
        "region_outage" => region_outage(),
        "photocopier" => photocopier(),
        "footnote" => footnote(),
        "whiteboard" => whiteboard(),
        "stakeholder" => stakeholder(),
        "cable_hydra" => cable_hydra(),
        "cable_head" => cable_head(),
        "slide_golem" => slide_golem(),
        "kpi_dashboard" => kpi_dashboard(),
        "pizza_overlord" => pizza_overlord(),
        "demo_gremlin" => demo_gremlin(),
        "energy_pyramid" => energy_pyramid(),
        "pizza_slice" => pizza_slice(),
        "apple" => apple(),
        "hallucination" => hallucination(),
        "release_halo" => release_halo(),
        "agi_prototype" => agi_prototype(),
        "scooter" => scooter(),
        "unreviewed_pr" => unreviewed_pr(),
        "demo_bug" => demo_bug(),
        "agi_beta" => agi_beta(),
        "drone" => drone(),
        "goose" => goose(),
        "cat" => cat(),
        "pizza" => pizza(),
        "espresso_cart" => espresso_cart(),
        "parking_barrier" => parking_barrier(),
        k if k.starts_with("pet_") => pet(&k[4..]),
        _ => vec![Part::cube(
            [0.0, 0.5, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        )],
    }
}

/// A stalk of cable: `n` cylinders that rise and curl over towards `+X` (`side` flips it), each
/// a little thinner, ending in a plug with a red eye. Parts are in the rig's local frame.
fn stalk(base: [f32; 3], side: f32, n: usize, len: f32, thick: f32, curl: f32) -> Vec<Part> {
    let mut v = Vec::new();
    let (mut x, mut y) = (base[0], base[1]);
    let mut t = 8.0_f32;
    for k in 0..n {
        let r = (t.to_radians().sin() * side, t.to_radians().cos());
        let d = thick * (1.0 - 0.1 * k as f32);
        let c = [x + r.0 * len / 2.0, y + r.1 * len / 2.0, base[2]];
        let col = if k % 3 == 1 {
            [0.8, 0.1, 0.1]
        } else {
            [0.06, 0.06, 0.07]
        };
        v.push(Part::cyl(c, d, len, col).rot([0.0, 0.0, -t * side]));
        x += r.0 * len;
        y += r.1 * len;
        t += curl;
    }
    v.push(Part::cube([x, y, base[2]], [0.3, 0.2, 0.2], STEEL).rot([0.0, 0.0, -t * side]));
    v.push(
        Part::ball(
            [x + 0.1 * side, y - 0.05, base[2] - 0.1],
            0.1,
            [1.0, 0.1, 0.1],
        )
        .glow(3.5),
    );
    v
}

/// Trash: a rack with cable arms that have come loose and reach for you.
fn tentacle_rack() -> Vec<Part> {
    let mut v = overheated_rack();
    v.extend(stalk([-0.4, 1.2, -0.1], -1.0, 4, 0.5, 0.12, 22.0));
    v.extend(stalk([0.4, 1.25, -0.1], 1.0, 4, 0.55, 0.12, 24.0));
    v.extend(stalk([0.0, 1.25, 0.2], 1.0, 3, 0.45, 0.1, 30.0));
    v
}

/// Boss of the Cold Aisle: a light-grey server monolith, taller than any raid, mid-update. The
/// face is one big update screen ("Working on updates, 99%", a progress bar that has not moved
/// since Tuesday, "Don't turn off your computer"), warning tape across the belly, and the update
/// icons turning on top and at the shoulders. The case is light so it never reads as a black slab.
fn patch_tuesday() -> Vec<Part> {
    const RED: [f32; 3] = [1.0, 0.08, 0.08];
    const GREEN: [f32; 3] = [0.1, 1.0, 0.3];
    const CASE: [f32; 3] = [0.42, 0.45, 0.52];
    const PANEL: [f32; 3] = [0.3, 0.32, 0.38];
    const TAPE: [f32; 3] = [1.0, 0.8, 0.05];
    const UPDATE: [f32; 3] = [0.25, 0.65, 1.0];
    let mut v = vec![
        // The chassis, a plinth and the flank panels.
        Part::cube([0.0, 2.0, 0.0], [2.8, 3.8, 1.5], CASE),
        Part::cube([0.0, 0.15, 0.0], [3.1, 0.3, 1.9], DARK),
        Part::cube([-1.42, 2.0, 0.0], [0.06, 3.4, 1.3], PANEL),
        Part::cube([1.42, 2.0, 0.0], [0.06, 3.4, 1.3], PANEL),
        // The beacon.
        Part::cyl([0.0, 4.05, 0.0], 0.5, 0.18, DARK),
        Part::cyl([0.0, 4.25, 0.0], 0.36, 0.26, UPDATE).glow(3.0),
    ];
    // The face is on both the front and the back of the case: a boss turns to whoever it fights,
    // and from behind a monolith with only a screen on one side is a plain slab.
    for s in [-1.0_f32, 1.0] {
        v.extend([
            Part::cube([0.0, 2.85, 0.76 * s], [2.5, 2.1, 0.08], PANEL),
            Part::screen(
                [0.0, 2.85, 0.82 * s],
                [2.2, 1.8, 0.04],
                if s < 0.0 { "patch" } else { "patch_back" },
            ),
            // The eyes above the screen: two status lamps stuck on "busy".
            Part::cube([-0.6, 3.95, 0.76 * s], [0.5, 0.12, 0.05], RED).glow(3.0),
            Part::cube([0.6, 3.95, 0.76 * s], [0.5, 0.12, 0.05], RED).glow(3.0),
            // A power button the size of a head: the one thing it forbids you to press.
            Part::cyl([0.0, 1.35, 0.8 * s], 0.5, 0.1, [0.9, 0.9, 0.95])
                .rot([90.0, 0.0, 0.0])
                .glow(1.2),
            Part::cube([0.0, 1.35, 0.86 * s], [0.08, 0.3, 0.02], RED).glow(3.0),
        ]);
        // Warning tape across the belly: alternating yellow and dark slashes.
        for k in 0..6 {
            let c = if k % 2 == 0 { TAPE } else { DARK };
            v.push(
                Part::cube(
                    [-1.2 + k as f32 * 0.22, 0.75, 0.78 * s],
                    [0.16, 0.38, 0.03],
                    c,
                )
                .rot([0.0, 0.0, 25.0])
                .glow(if k % 2 == 0 { 0.5 } else { 0.0 }),
            );
        }
        // Rack LEDs down both sides of the screen.
        for row in 0..4 {
            for side in [-1.0_f32, 1.0] {
                v.push(
                    Part::cube(
                        [side * 1.3, 1.7 + row as f32 * 0.5, 0.78 * s],
                        [0.08, 0.1, 0.03],
                        if (row + side as i32) % 3 == 0 {
                            GREEN
                        } else {
                            RED
                        },
                    )
                    .glow(2.4)
                    .bob(0.012, 5.0 + row as f32),
                );
            }
        }
    }
    // Update icons: a ring of eight dashes that turns, three of them (shoulders and crown).
    for (x, y, size) in [(-1.75, 3.5, 1.0_f32), (1.75, 3.5, 1.0), (0.0, 4.9, 1.3)] {
        let mut dashes = Vec::new();
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            dashes.push(
                Part::cube(
                    [a.cos() * 0.32 * size, a.sin() * 0.32 * size, 0.0],
                    [0.1 * size, 0.1 * size, 0.06],
                    if k < 6 { UPDATE } else { [0.1, 0.2, 0.5] },
                )
                .glow(if k < 6 { 3.0 } else { 0.0 })
                .rot([0.0, 0.0, a.to_degrees()]),
            );
        }
        v.push(
            Part::cube([x, y, -0.5], [0.0, 0.0, 0.0], DARK)
                .with(dashes)
                .spin(Vec3::Z, 3.0)
                .bob(0.06, 1.8),
        );
    }
    // Two cable arms, ready to reboot something.
    for side in [-1.0_f32, 1.0] {
        v.push(
            Part::cyl([side * 1.8, 2.0, -0.2], 0.3, 1.8, [0.06, 0.06, 0.07]).rot([
                0.0,
                0.0,
                side * 14.0,
            ]),
        );
        v.push(Part::cube(
            [side * 2.1, 1.0, -0.3],
            [0.55, 0.5, 0.45],
            STEEL,
        ));
    }
    v
}

/// Boss of the Review Stage: a standing review form, a giant clipboard with a binder clip on top, a
/// rating card for a face (a big 3) and the whole room's cards turning round it. Everyone is a
/// three; the pencil is the only thing that disagrees.
fn perf_review() -> Vec<Part> {
    const PAPER: [f32; 3] = [0.93, 0.91, 0.82];
    const BOARD: [f32; 3] = [0.45, 0.3, 0.18];
    const INK: [f32; 3] = [0.12, 0.2, 0.5];
    let mut v = vec![
        Part::cube([0.0, 2.2, 0.0], [2.6, 3.8, 0.3], BOARD),
        Part::cube([0.0, 2.2, -0.17], [2.3, 3.5, 0.04], PAPER),
        Part::screen([0.0, 3.0, -0.2], [1.7, 1.7, 0.03], "review"),
        Part::cube([0.0, 2.2, 0.17], [2.3, 3.5, 0.04], PAPER),
        Part::screen([0.0, 3.0, 0.2], [1.7, 1.7, 0.03], "review_back"),
        // Form lines under the card, the last one a signature that is not yours.
        Part::cube([0.0, 1.85, -0.2], [1.8, 0.07, 0.02], INK),
        Part::cube([0.0, 1.6, -0.2], [1.8, 0.07, 0.02], INK),
        Part::cube([0.0, 1.35, -0.2], [1.8, 0.07, 0.02], INK),
        Part::cube([-0.3, 0.95, -0.2], [1.2, 0.05, 0.02], [0.6, 0.1, 0.1]),
        // The clip.
        Part::cube([0.0, 4.15, -0.05], [1.0, 0.3, 0.3], STEEL),
        Part::cyl([0.0, 4.4, -0.05], 0.3, 0.12, STEEL).rot([0.0, 0.0, 90.0]),
        // Two legs and a pair of tiny arms, because it came to the meeting in person.
        Part::cyl([-0.6, 0.35, 0.0], 0.28, 0.7, DARK),
        Part::cyl([0.6, 0.35, 0.0], 0.28, 0.7, DARK),
        Part::cyl([-1.55, 2.2, -0.1], 0.2, 1.6, DARK).rot([0.0, 0.0, 22.0]),
        Part::cyl([1.55, 2.2, -0.1], 0.2, 1.6, DARK).rot([0.0, 0.0, -22.0]),
        // The pencil, yellow, in its fist.
        Part::cyl([1.9, 1.3, -0.4], 0.1, 1.2, [1.0, 0.8, 0.1]).rot([0.0, 0.0, 35.0]),
        Part::cyl([2.25, 0.8, -0.4], 0.1, 0.18, [0.1, 0.1, 0.1]).rot([0.0, 0.0, 35.0]),
    ];
    // Rating cards: seven white slabs on a slow orbit, all the same.
    let mut cards = Vec::new();
    for k in 0..7 {
        let a = k as f32 * std::f32::consts::TAU / 7.0;
        cards.push(
            Part::cube(
                [a.cos() * 2.5, 0.0, a.sin() * 2.5],
                [0.5, 0.7, 0.04],
                [0.96, 0.95, 0.88],
            )
            .glow(0.5)
            .rot([0.0, -a.to_degrees() + 90.0, 0.0]),
        );
    }
    v.push(
        Part::cube([0.0, 2.6, 0.0], [0.0, 0.0, 0.0], DARK)
            .with(cards)
            .spin(Vec3::Y, 0.6),
    );
    v
}

/// Boss of the Scrape Pit: a vent-grille spider of a queen. A flat black thorax with a lit
/// crawler-eye array, eight bent legs of conduit, a crown of antennas that receive everything, a
/// swollen abdomen full of folders with a green status light that never goes red, and four
/// drones that orbit her like a very polite retinue.
fn scraper_queen() -> Vec<Part> {
    const SHELL: [f32; 3] = [0.16, 0.18, 0.2];
    const GRILLE: [f32; 3] = [0.3, 0.34, 0.36];
    const GREEN: [f32; 3] = [0.3, 1.0, 0.55];
    let mut v = vec![
        // Thorax and head.
        Part::cube([0.0, 1.0, 0.0], [1.3, 0.7, 1.5], SHELL),
        Part::cube([0.0, 1.0, -0.82], [0.9, 0.55, 0.4], GRILLE),
        // Eyes: a bar of green crawler lights across the face.
        Part::cube([0.0, 1.1, -1.05], [0.8, 0.1, 0.04], GREEN).glow(3.5),
        Part::cube([-0.25, 0.92, -1.05], [0.14, 0.14, 0.04], GREEN).glow(3.0),
        Part::cube([0.25, 0.92, -1.05], [0.14, 0.14, 0.04], GREEN).glow(3.0),
        // The abdomen: a big pale sac of folders, a lit status strip along it.
        Part::ball([0.0, 1.05, 1.35], 1.6, [0.82, 0.8, 0.72]),
        Part::cube([0.0, 1.9, 1.25], [0.12, 0.05, 1.2], GREEN).glow(2.5),
        Part::cube([0.0, 1.55, 1.9], [1.1, 0.1, 0.6], [0.95, 0.93, 0.85]).rot([-18.0, 0.0, 0.0]),
        Part::cube([0.0, 1.35, 2.0], [1.0, 0.1, 0.6], [0.9, 0.9, 0.8]).rot([-24.0, 0.0, 4.0]),
        // Crown of antennas.
        Part::cyl([0.0, 1.9, -0.6], 0.04, 1.0, [0.6, 0.62, 0.66]),
        Part::cyl([-0.3, 1.8, -0.55], 0.04, 0.8, [0.6, 0.62, 0.66]).rot([0.0, 0.0, 14.0]),
        Part::cyl([0.3, 1.8, -0.55], 0.04, 0.8, [0.6, 0.62, 0.66]).rot([0.0, 0.0, -14.0]),
        Part::ball([0.0, 2.45, -0.6], 0.12, GREEN)
            .glow(4.0)
            .bob(0.04, 5.0),
    ];
    // Eight conduit legs: a thigh up and out, a shin down to the floor.
    for side in [-1.0_f32, 1.0] {
        for k in 0..4 {
            let z = -0.55 + k as f32 * 0.5;
            let spread = 0.35 + 0.06 * k as f32;
            v.push(
                Part::cyl([side * (0.95 + spread), 1.45, z], 0.14, 1.1, SHELL).rot([
                    0.0,
                    0.0,
                    -side * 62.0,
                ]),
            );
            v.push(
                Part::cyl([side * (1.9 + spread), 0.75, z], 0.1, 1.55, GRILLE).rot([
                    0.0,
                    0.0,
                    side * 14.0,
                ]),
            );
            v.push(Part::ball(
                [side * (2.1 + spread), 0.05, z],
                0.18,
                [0.9, 0.92, 0.95],
            ));
        }
    }
    // The retinue: four drones that circle her.
    let mut orbit = Vec::new();
    for k in 0..4 {
        let a = k as f32 * TAU / 4.0;
        orbit.push(
            Part::cube(
                [a.cos() * 2.4, 0.0, a.sin() * 2.4],
                [0.4, 0.1, 0.4],
                [0.92, 0.92, 0.9],
            )
            .rot([0.0, -a.to_degrees(), 0.0])
            .bob(0.1, 3.0),
        );
        orbit.push(
            Part::ball([a.cos() * 2.4, -0.05, a.sin() * 2.4], 0.1, [1.0, 0.1, 0.1]).glow(3.0),
        );
    }
    v.push(
        Part::cube([0.0, 2.2, 0.0], [0.0, 0.0, 0.0], SHELL)
            .with(orbit)
            .spin(Vec3::Y, 0.9),
    );
    v
}

/// A cooling fan that got up and followed you: a drum on its side with spinning blades, two
/// hot eyes on the hub and a power cable it drags.
fn fan_elemental() -> Vec<Part> {
    const CASE: [f32; 3] = [0.28, 0.3, 0.34];
    let mut v = vec![
        Part::cyl([0.0, 1.2, 0.0], 1.7, 0.35, CASE).rot([90.0, 0.0, 0.0]),
        Part::cyl([0.0, 1.2, -0.17], 1.35, 0.05, DARK).rot([90.0, 0.0, 0.0]),
        Part::cyl([0.0, 1.2, -0.24], 0.4, 0.14, STEEL).rot([90.0, 0.0, 0.0]),
        Part::ball([-0.1, 1.28, -0.34], 0.1, HOT).glow(4.0),
        Part::ball([0.1, 1.28, -0.34], 0.1, HOT).glow(4.0),
        Part::cube([0.0, 0.2, 0.2], [0.5, 0.4, 0.5], CASE),
        Part::cube([0.0, 0.05, 0.9], [0.1, 0.08, 1.2], DARK).rot([0.0, 18.0, 0.0]),
    ];
    // Four blades on a hub that spins, the glow of a motor that is not rated for this.
    let blades: Vec<Part> = (0..4)
        .map(|k| {
            Part::cube(
                [0.0, 0.0, 0.0],
                [1.5, 0.28, 0.04],
                if k % 2 == 0 { STEEL } else { CYAN },
            )
            .rot([0.0, 0.0, k as f32 * 45.0])
        })
        .collect();
    v.push(
        Part::cube([0.0, 1.2, -0.27], [0.0, 0.0, 0.0], STEEL)
            .with(blades)
            .spin(Vec3::Z, 9.0),
    );
    v
}

/// A diesel generator that learned to walk: a boxy body with exhaust stacks, a fuel can for a
/// backpack, a gauge face with the needle in the red and two tracks.
fn generator_golem() -> Vec<Part> {
    const BODY: [f32; 3] = [0.9, 0.62, 0.12];
    const GREY: [f32; 3] = [0.3, 0.3, 0.32];
    vec![
        Part::cube([-0.65, 0.3, 0.0], [0.5, 0.6, 1.6], DARK),
        Part::cube([0.65, 0.3, 0.0], [0.5, 0.6, 1.6], DARK),
        Part::cube([0.0, 1.3, 0.0], [1.9, 1.4, 1.5], BODY),
        Part::cube([0.0, 2.1, 0.0], [1.4, 0.4, 1.2], GREY),
        Part::cyl([-0.5, 2.7, 0.3], 0.26, 1.0, GREY),
        Part::cyl([0.5, 2.7, 0.3], 0.26, 1.0, GREY),
        Part::ball([-0.5, 3.3, 0.3], 0.35, [0.2, 0.2, 0.22]).bob(0.12, 1.4),
        Part::ball([0.5, 3.2, 0.3], 0.3, [0.25, 0.25, 0.27]).bob(0.14, 1.9),
        // The face: a gauge in the red and two work lamps.
        Part::cyl([0.0, 1.5, -0.78], 0.8, 0.06, [0.95, 0.95, 0.9]).rot([90.0, 0.0, 0.0]),
        Part::cube([0.12, 1.6, -0.83], [0.04, 0.5, 0.02], [1.0, 0.1, 0.1]).rot([0.0, 0.0, 40.0]),
        Part::ball([-0.55, 1.95, -0.78], 0.22, [1.0, 0.9, 0.4]).glow(3.5),
        Part::ball([0.55, 1.95, -0.78], 0.22, [1.0, 0.9, 0.4]).glow(3.5),
        // Hazard stripe, a fuel can and a cable for an arm.
        Part::cube([0.0, 0.75, -0.77], [1.9, 0.14, 0.04], [0.15, 0.15, 0.15]),
        Part::cube([0.0, 1.3, 0.95], [0.8, 1.0, 0.4], [0.75, 0.15, 0.1]),
        Part::cyl([1.2, 1.2, 0.0], 0.2, 1.3, DARK).rot([0.0, 0.0, 12.0]),
        Part::cyl([-1.2, 1.2, 0.0], 0.2, 1.3, DARK).rot([0.0, 0.0, -12.0]),
    ]
}

/// A reserved instance: a tidy little kiosk with a clock for a face that never stops, a
/// three-year calendar flag and a price tag on a string.
fn reserved_bot() -> Vec<Part> {
    const CASE: [f32; 3] = [0.82, 0.86, 0.95];
    const BLUE: [f32; 3] = [0.15, 0.3, 0.72];
    vec![
        Part::cube([0.0, 0.85, 0.0], [0.9, 1.0, 0.7], CASE),
        Part::cube([0.0, 0.2, 0.0], [0.8, 0.4, 0.6], BLUE),
        Part::cyl([0.0, 1.1, -0.37], 0.6, 0.06, [0.97, 0.97, 0.94]).rot([90.0, 0.0, 0.0]),
        Part::cube([0.0, 1.15, -0.42], [0.04, 0.24, 0.02], DARK),
        Part::cube([0.07, 1.1, -0.42], [0.2, 0.04, 0.02], [1.0, 0.1, 0.1]),
        Part::cube([0.0, 1.55, 0.0], [0.5, 0.3, 0.4], BLUE),
        Part::ball([0.0, 1.55, -0.21], 0.12, CYAN).glow(3.0),
        Part::cyl([0.0, 1.9, 0.0], 0.04, 0.6, STEEL),
        Part::cube([0.17, 2.1, 0.0], [0.34, 0.22, 0.02], [1.0, 0.85, 0.2]).bob(0.05, 2.0),
        Part::cube([0.5, 0.9, -0.05], [0.1, 0.1, 0.55], CASE),
        Part::cube([-0.5, 0.9, -0.05], [0.1, 0.1, 0.55], CASE),
        Part::cube([0.5, 0.78, -0.4], [0.3, 0.22, 0.02], [0.98, 0.98, 0.9]),
    ]
}

/// An unmounted rack that walks: a tall cabinet with LED rows for a face, blade-server fists
/// and a mess of cable ends where its feet should be.
fn rack_golem() -> Vec<Part> {
    const FRAME: [f32; 3] = [0.14, 0.15, 0.18];
    let mut v = vec![
        Part::cube([0.0, 1.5, 0.0], [1.1, 2.6, 0.9], FRAME),
        Part::cube([0.0, 1.5, -0.46], [0.95, 2.4, 0.04], [0.2, 0.22, 0.26]),
        Part::cube([-0.8, 1.4, 0.0], [0.35, 0.35, 1.0], STEEL),
        Part::cube([0.8, 1.4, 0.0], [0.35, 0.35, 1.0], STEEL),
        Part::cube([-0.8, 1.9, 0.0], [0.2, 1.0, 0.25], FRAME).rot([0.0, 0.0, 15.0]),
        Part::cube([0.8, 1.9, 0.0], [0.2, 1.0, 0.25], FRAME).rot([0.0, 0.0, -15.0]),
        Part::cube([0.0, 0.1, 0.1], [1.0, 0.2, 0.9], [0.05, 0.05, 0.06]),
    ];
    for k in 0..9 {
        let y = 0.5 + k as f32 * 0.26;
        let c = if k % 3 == 1 { HOT } else { [0.2, 1.0, 0.5] };
        v.push(Part::cube([-0.25, y, -0.5], [0.12, 0.07, 0.02], c).glow(2.5));
        v.push(Part::cube(
            [0.15, y, -0.5],
            [0.4, 0.05, 0.02],
            [0.1, 0.1, 0.12],
        ));
    }
    // Cables out of the bottom, bobbing a little.
    for (x, r) in [(-0.35, 14.0), (0.0, -8.0), (0.35, 20.0)] {
        v.push(
            Part::cyl([x, 0.35, -0.5], 0.07, 0.8, DARK)
                .rot([r, 0.0, 0.0])
                .bob(0.03, 2.4),
        );
    }
    v
}

/// A pager: a beige slab with a green screen reading BEEP, an antenna, a belt clip and a
/// little halo of rings that go out whenever someone is on call.
fn pager() -> Vec<Part> {
    const CASE: [f32; 3] = [0.78, 0.74, 0.62];
    vec![
        Part::cube([0.0, 1.1, 0.0], [0.8, 1.0, 0.18], CASE).bob(0.06, 2.6),
        Part::cube([0.0, 1.3, -0.1], [0.6, 0.35, 0.02], [0.25, 0.9, 0.4])
            .glow(2.8)
            .bob(0.06, 2.6),
        Part::cube([-0.15, 0.85, -0.1], [0.12, 0.12, 0.03], [0.2, 0.2, 0.22]).bob(0.06, 2.6),
        Part::cube([0.15, 0.85, -0.1], [0.12, 0.12, 0.03], [0.2, 0.2, 0.22]).bob(0.06, 2.6),
        Part::cyl([0.3, 1.75, 0.0], 0.04, 0.5, STEEL).bob(0.06, 2.6),
        Part::cube([0.0, 1.1, 0.14], [0.2, 0.7, 0.06], STEEL).bob(0.06, 2.6),
        Part::cyl([0.0, 1.1, -0.2], 1.2, 0.02, [1.0, 0.3, 0.2])
            .glow(1.5)
            .rot([90.0, 0.0, 0.0])
            .bob(0.06, 2.6),
    ]
}

/// Boss of the Autoscale Floor: a tower of racks that keeps adding racks to itself. A tall
/// central cabinet, smaller copies bolted on at angles, green "scale up" arrows on top, a
/// gold ring of coins that spin round it (the bill) and a bank of LEDs that never goes red.
fn autoscaler() -> Vec<Part> {
    const FRAME: [f32; 3] = [0.12, 0.14, 0.2];
    const GREEN: [f32; 3] = [0.2, 1.0, 0.5];
    const GOLD: [f32; 3] = [1.0, 0.8, 0.2];
    let mut v = vec![
        Part::cube([0.0, 2.2, 0.0], [2.0, 4.4, 1.6], FRAME),
        Part::cube([0.0, 2.2, -0.82], [1.8, 4.1, 0.05], [0.18, 0.2, 0.28]),
        Part::cube([0.0, 4.6, 0.0], [1.6, 0.35, 1.3], [0.3, 0.34, 0.45]),
        // The face: three console panels, the middle one an eye.
        Part::cube([0.0, 3.4, -0.86], [1.5, 0.5, 0.03], [0.05, 0.08, 0.12]),
        Part::ball([0.0, 3.4, -0.9], 0.34, GREEN).glow(4.5),
        Part::cube([0.0, 2.7, -0.86], [1.5, 0.12, 0.03], GREEN).glow(2.5),
    ];
    // LED rows down the front.
    for k in 0..10 {
        let y = 0.4 + k as f32 * 0.22;
        v.push(Part::cube([-0.55, y, -0.86], [0.12, 0.06, 0.02], GREEN).glow(2.5));
        v.push(Part::cube([-0.25, y, -0.86], [0.12, 0.06, 0.02], [0.3, 0.7, 1.0]).glow(2.0));
    }
    // The replicas bolted on: smaller racks round the base, each a little less finished.
    for (k, (x, z, s)) in [(-1.7, 0.2, 0.7), (1.75, -0.1, 0.62), (0.0, 1.45, 0.55)]
        .into_iter()
        .enumerate()
    {
        v.push(Part::cube(
            [x, 1.6 * s, z],
            [1.4 * s, 3.2 * s, 1.2 * s],
            FRAME,
        ));
        v.push(Part::cube(
            [x, 1.5 * s, z - 0.62 * s],
            [1.2 * s, 2.8 * s, 0.03],
            [0.18, 0.2, 0.28],
        ));
        v.push(
            Part::ball([x, 2.4 * s, z - 0.66 * s], 0.2, GREEN)
                .glow(3.5)
                .bob(0.03, 2.0 + k as f32),
        );
    }
    // Scale-up arrows on the roof, rising and fading in turns.
    for (x, ph) in [(-0.5, 0.0), (0.0, 1.0), (0.5, 2.0)] {
        v.push(
            Part::cube([x, 5.1, 0.0], [0.12, 0.7, 0.12], GREEN)
                .glow(3.0)
                .bob(0.25, 2.0 + ph),
        );
        v.push(
            Part::cube([x, 5.5, 0.0], [0.4, 0.14, 0.14], GREEN)
                .rot([0.0, 0.0, 30.0])
                .glow(3.0)
                .bob(0.25, 2.0 + ph),
        );
    }
    // The bill: gold discs that circle the tower, one for every month it has been running.
    let mut coins = Vec::new();
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        coins.push(
            Part::cyl([a.cos() * 2.9, 0.0, a.sin() * 2.9], 0.55, 0.08, GOLD)
                .rot([90.0, -a.to_degrees(), 0.0])
                .glow(1.2)
                .bob(0.1, 2.5 + k as f32 * 0.3),
        );
    }
    v.push(
        Part::cube([0.0, 2.0, 0.0], [0.0, 0.0, 0.0], GOLD)
            .with(coins)
            .spin(Vec3::Y, 0.7),
    );
    v
}

/// Boss of the Server Cloud: a thunderhead of white balls with a status light for an eye (red,
/// whatever the status page says), a lightning bolt, and five small platforms orbiting it, all
/// green but the one that is having a moment.
fn region_outage() -> Vec<Part> {
    const CLOUD: [f32; 3] = [0.93, 0.95, 0.99];
    const GREY: [f32; 3] = [0.62, 0.66, 0.76];
    const RED: [f32; 3] = [1.0, 0.12, 0.1];
    let mut v = vec![
        Part::ball([0.0, 3.0, 0.0], 4.2, CLOUD).bob(0.1, 1.2),
        Part::ball([-2.0, 2.5, 0.2], 3.0, CLOUD).bob(0.12, 1.5),
        Part::ball([2.1, 2.6, -0.2], 3.2, CLOUD).bob(0.1, 1.7),
        Part::ball([0.4, 4.4, 0.1], 2.8, GREY).bob(0.14, 1.3),
        Part::ball([-1.2, 1.4, 0.1], 2.4, GREY),
        Part::ball([1.4, 1.4, 0.0], 2.2, GREY),
        // The eye: a socket and a red status light, three bars of "503" below it.
        Part::ball([0.0, 3.2, -1.95], 1.1, [0.1, 0.1, 0.14]),
        Part::ball([0.0, 3.2, -2.45], 0.62, RED)
            .glow(6.0)
            .bob(0.05, 3.0),
        Part::cube([-0.35, 2.4, -2.1], [0.25, 0.4, 0.04], RED).glow(2.5),
        Part::cube([0.0, 2.4, -2.1], [0.25, 0.4, 0.04], RED).glow(2.5),
        Part::cube([0.35, 2.4, -2.1], [0.25, 0.4, 0.04], RED).glow(2.5),
        // A lightning bolt out of the bottom: two bars at an angle and a tip.
        Part::cube([0.3, 0.7, 0.0], [0.28, 1.4, 0.2], [1.0, 0.95, 0.3])
            .glow(4.0)
            .rot([0.0, 0.0, 16.0]),
        Part::cube([0.0, -0.2, 0.0], [0.28, 1.1, 0.2], [1.0, 0.95, 0.3])
            .glow(4.0)
            .rot([0.0, 0.0, -18.0]),
    ];
    // Five tiny regions on a ring: four green, one red (the one that went first).
    let mut ring = Vec::new();
    for k in 0..5 {
        let a = k as f32 * TAU / 5.0;
        let c = if k == 0 { RED } else { [0.2, 1.0, 0.5] };
        ring.push(Part::cyl(
            [a.cos() * 4.8, 0.0, a.sin() * 4.8],
            0.9,
            0.18,
            GREY,
        ));
        ring.push(Part::ball([a.cos() * 4.8, 0.2, a.sin() * 4.8], 0.22, c).glow(3.5));
    }
    v.push(
        Part::cube([0.0, 1.6, 0.0], [0.0, 0.0, 0.0], GREY)
            .with(ring)
            .spin(Vec3::Y, 0.6),
    );
    v
}

/// Boss of the Fine-Tuning Chamber: a photocopier on its last leg, copied from a copier. The lid is
/// up and the glass shows a smudge of a face (generation upon generation), a ream of beige
/// output hangs from the tray, a status panel reads "TRAINING LOSS 0.003" and two smaller
/// photocopiers, each a blurrier copy of the first, orbit it.
fn photocopier() -> Vec<Part> {
    const CASE: [f32; 3] = [0.78, 0.72, 0.6];
    const BEIGE: [f32; 3] = [0.7, 0.64, 0.52];
    const PAPER: [f32; 3] = [0.92, 0.9, 0.82];
    let mut v = vec![
        // The body, the lid and the glass.
        Part::cube([0.0, 0.9, 0.0], [2.2, 1.8, 1.3], CASE),
        Part::cube([0.0, 1.95, 0.1], [2.2, 0.18, 1.2], BEIGE).rot([-14.0, 0.0, 0.0]),
        Part::cube([0.0, 1.82, -0.05], [1.9, 0.04, 1.0], [0.3, 0.45, 0.5]).glow(0.8),
        // The smudge on the glass: a face that has been copied too often.
        Part::ball([0.0, 1.87, -0.1], 0.5, [0.74, 0.68, 0.58]),
        Part::ball([-0.12, 1.9, -0.25], 0.07, [0.5, 0.45, 0.38]),
        Part::ball([0.12, 1.9, -0.25], 0.07, [0.5, 0.45, 0.38]),
        // Control panel: lit.
        Part::cube([0.0, 1.35, -0.67], [1.1, 0.4, 0.04], [0.08, 0.1, 0.08]),
        Part::cube([-0.1, 1.4, -0.7], [0.8, 0.12, 0.02], [0.3, 1.0, 0.5]).glow(2.5),
        Part::cube([0.0, 1.25, -0.7], [0.5, 0.05, 0.02], [1.0, 0.8, 0.2]).glow(2.0),
        // The tray and its output: a ream of beige that keeps on coming.
        Part::cube([0.0, 0.55, -0.95], [1.5, 0.08, 0.7], BEIGE),
        Part::cube([0.0, 0.64, -0.95], [1.2, 0.1, 0.55], PAPER),
        Part::cube([0.0, 0.8, -0.97], [1.1, 0.1, 0.5], [0.88, 0.84, 0.76]),
        Part::cube([0.0, 0.94, -0.99], [1.0, 0.1, 0.45], [0.84, 0.8, 0.7]),
        Part::cube([0.0, 1.07, -1.01], [0.9, 0.1, 0.4], [0.8, 0.76, 0.66]),
        // Feet.
        Part::cube([-0.9, 0.06, 0.4], [0.3, 0.12, 0.3], [0.2, 0.2, 0.2]),
        Part::cube([0.9, 0.06, 0.4], [0.3, 0.12, 0.3], [0.2, 0.2, 0.2]),
        Part::cube([-0.9, 0.06, -0.4], [0.3, 0.12, 0.3], [0.2, 0.2, 0.2]),
        Part::cube([0.9, 0.06, -0.4], [0.3, 0.12, 0.3], [0.2, 0.2, 0.2]),
        // A sticker: "OUT OF TONER" has been out of toner since generation 2.
        Part::cube([0.9, 1.3, -0.67], [0.3, 0.2, 0.02], [1.0, 0.85, 0.1]),
    ];
    // Two generations of copy circling the original, each a bit duller and a bit smaller.
    let mut orbit = Vec::new();
    for (k, (size, tint)) in [(0.8_f32, 0.9_f32), (0.6, 0.78)].into_iter().enumerate() {
        let a = k as f32 * TAU / 2.0;
        let c = [CASE[0] * tint, CASE[1] * tint, CASE[2] * tint];
        orbit.push(Part::cube(
            [a.cos() * 2.8, 0.0, a.sin() * 2.8],
            [size, size * 0.8, size * 0.6],
            c,
        ));
        orbit.push(
            Part::cube(
                [a.cos() * 2.8, size * 0.45, a.sin() * 2.8],
                [size * 0.8, 0.06, size * 0.5],
                [PAPER[0] * tint, PAPER[1] * tint, PAPER[2] * tint],
            )
            .bob(0.05, 2.0),
        );
    }
    v.push(
        Part::cube([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], CASE)
            .with(orbit)
            .spin(Vec3::Y, 0.5),
    );
    v
}

/// A footnote: a speech-bubble page with a superscript number, a little halo of "[citation needed]"
/// bars and a tail. Stands, glares, bobs.
fn footnote() -> Vec<Part> {
    const PAGE: [f32; 3] = [0.96, 0.95, 0.9];
    const INK: [f32; 3] = [0.15, 0.2, 0.5];
    vec![
        Part::cube([0.0, 1.0, 0.0], [0.9, 1.2, 0.08], PAGE).bob(0.06, 2.4),
        Part::cube([0.0, 1.0, -0.05], [0.7, 0.05, 0.02], INK).bob(0.06, 2.4),
        Part::cube([0.0, 0.85, -0.05], [0.7, 0.05, 0.02], INK).bob(0.06, 2.4),
        Part::cube([0.0, 0.7, -0.05], [0.5, 0.05, 0.02], INK).bob(0.06, 2.4),
        // The superscript: a bright "1" at the top corner.
        Part::cube([0.28, 1.45, -0.05], [0.08, 0.26, 0.03], [0.9, 0.2, 0.2])
            .glow(2.5)
            .bob(0.06, 2.4),
        Part::cube([0.22, 1.35, -0.05], [0.12, 0.05, 0.03], [0.9, 0.2, 0.2])
            .glow(2.5)
            .bob(0.06, 2.4),
        // Eyes: two cold dots, quite sure.
        Part::ball([-0.14, 1.22, -0.06], 0.12, [0.1, 0.1, 0.12]).bob(0.06, 2.4),
        Part::ball([0.14, 1.22, -0.06], 0.12, [0.1, 0.1, 0.12]).bob(0.06, 2.4),
        Part::cube([0.0, 0.25, 0.0], [0.12, 0.3, 0.12], PAGE).rot([0.0, 0.0, 20.0]),
    ]
}

/// The Lab's whiteboard: a board on an easel with three phases on it, one of them a question mark.
fn whiteboard() -> Vec<Part> {
    const BOARD: [f32; 3] = [0.97, 0.97, 0.96];
    const INK: [f32; 3] = [0.15, 0.2, 0.55];
    vec![
        Part::cube([0.0, 1.3, 0.0], [1.9, 1.2, 0.06], BOARD),
        Part::cube([0.0, 1.3, 0.04], [2.0, 1.3, 0.04], [0.6, 0.6, 0.62]),
        Part::cube([-0.6, 1.5, -0.04], [0.5, 0.05, 0.02], INK),
        Part::cube([-0.6, 1.3, -0.04], [0.5, 0.05, 0.02], INK),
        Part::cube([0.1, 1.5, -0.04], [0.7, 0.05, 0.02], INK),
        Part::cube([0.5, 1.1, -0.04], [0.2, 0.2, 0.02], [0.8, 0.15, 0.15]),
        Part::cyl([-0.7, 0.6, 0.1], 0.06, 1.2, [0.5, 0.5, 0.52]).rot([14.0, 0.0, 0.0]),
        Part::cyl([0.7, 0.6, 0.1], 0.06, 1.2, [0.5, 0.5, 0.52]).rot([14.0, 0.0, 0.0]),
        Part::cyl([0.0, 0.6, 0.45], 0.06, 1.2, [0.5, 0.5, 0.52]).rot([-26.0, 0.0, 0.0]),
        Part::cube([0.0, 0.72, -0.12], [1.6, 0.05, 0.1], [0.6, 0.6, 0.62]),
    ]
}

/// An HR bot: a white egg of a body, a lanyard, a glowing smile and one arm that holds a form.
fn hr_bot() -> Vec<Part> {
    const SHELL: [f32; 3] = [0.92, 0.93, 0.97];
    vec![
        Part::cyl([0.0, 0.55, 0.0], 0.75, 0.9, SHELL),
        Part::ball([0.0, 0.5, 0.0], 0.9, SHELL),
        Part::ball([0.0, 1.15, 0.0], 0.62, SHELL),
        Part::cube([0.0, 1.17, -0.3], [0.5, 0.2, 0.04], DARK),
        Part::cube([-0.12, 1.2, -0.33], [0.1, 0.1, 0.02], CYAN).glow(3.0),
        Part::cube([0.12, 1.2, -0.33], [0.1, 0.1, 0.02], CYAN).glow(3.0),
        Part::cube([0.0, 1.1, -0.33], [0.3, 0.04, 0.02], CYAN).glow(2.5),
        Part::cube([0.0, 0.7, -0.4], [0.12, 0.36, 0.03], [0.85, 0.1, 0.1]),
        Part::cube([0.0, 0.48, -0.41], [0.26, 0.2, 0.03], [0.95, 0.95, 0.95]),
        Part::cyl([0.0, 1.55, 0.0], 0.05, 0.3, STEEL),
        Part::ball([0.0, 1.74, 0.0], 0.14, [1.0, 0.3, 0.3])
            .glow(3.0)
            .bob(0.03, 4.0),
        Part::cube([0.55, 0.65, -0.3], [0.36, 0.46, 0.04], [0.95, 0.92, 0.8])
            .rot([0.0, 0.0, -12.0]),
        Part::cyl([0.0, 0.08, 0.0], 0.9, 0.16, DARK),
    ]
}

/// A stakeholder: a speech bubble with a question mark and a tail, bobbing at head height.
fn stakeholder() -> Vec<Part> {
    const BUBBLE: [f32; 3] = [0.97, 0.97, 0.99];
    vec![
        Part::ball([0.0, 1.5, 0.0], 1.5, BUBBLE),
        Part::cube([0.0, 0.7, -0.1], [0.3, 0.5, 0.3], BUBBLE).rot([0.0, 0.0, 30.0]),
        // The question mark: an arc of cubes, a stem and a dot.
        Part::cube([-0.2, 1.95, -0.74], [0.18, 0.18, 0.05], [0.15, 0.2, 0.55]),
        Part::cube([0.0, 2.05, -0.74], [0.2, 0.18, 0.05], [0.15, 0.2, 0.55]),
        Part::cube([0.2, 1.9, -0.74], [0.18, 0.2, 0.05], [0.15, 0.2, 0.55]),
        Part::cube([0.1, 1.6, -0.74], [0.18, 0.2, 0.05], [0.15, 0.2, 0.55]),
        Part::cube([0.0, 1.35, -0.74], [0.18, 0.2, 0.05], [0.15, 0.2, 0.55]),
        Part::cube([0.0, 1.0, -0.74], [0.2, 0.2, 0.05], [0.15, 0.2, 0.55]),
        Part::cyl([0.0, 0.1, 0.0], 0.8, 0.12, [0.2, 0.2, 0.3]),
    ]
}

/// Boss of the Patch Panel: a black rack wall that has grown five cable heads, each with a
/// red eye. The heads sway; the cables pool on the floor in front of it.
fn cable_hydra() -> Vec<Part> {
    const RED: [f32; 3] = [1.0, 0.08, 0.08];
    let mut v = vec![
        Part::cube([0.0, 1.7, 0.0], [3.2, 3.0, 1.5], [0.08, 0.08, 0.1]),
        Part::cube([0.0, 1.7, -0.78], [3.0, 2.8, 0.04], [0.15, 0.15, 0.18]),
        Part::cube([0.0, 0.1, 0.0], [3.4, 0.2, 1.8], DARK),
        Part::cube([-0.7, 2.6, -0.8], [0.6, 0.2, 0.03], RED).glow(3.5),
        Part::cube([0.7, 2.6, -0.8], [0.6, 0.2, 0.03], RED).glow(3.5),
        Part::cube([0.0, 1.0, -0.8], [2.0, 0.3, 0.03], HOT).glow(2.5),
    ];
    for row in 0..4 {
        for col in 0..6 {
            let m = if (row * 5 + col * 3) % 8 == 0 {
                [0.1, 1.0, 0.3]
            } else {
                RED
            };
            v.push(
                Part::cube(
                    [-1.2 + col as f32 * 0.48, 0.5 + row as f32 * 0.4, -0.8],
                    [0.24, 0.1, 0.02],
                    m,
                )
                .glow(2.2)
                .bob(0.01, 5.0 + (row + col) as f32),
            );
        }
    }
    for (k, x) in [-1.3_f32, -0.65, 0.0, 0.65, 1.3].into_iter().enumerate() {
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let head = stalk([x, 3.1, -0.2], side, 4, 0.7, 0.18, 30.0 + k as f32 * 3.0);
        v.push(
            Part::cube([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], DARK)
                .with(head)
                .bob(0.05, 1.5 + k as f32 * 0.3),
        );
    }
    for (x, z, yaw, c) in [
        (-1.0, -1.6, 20.0, [0.06, 0.06, 0.07]),
        (0.6, -1.9, -30.0, RED),
        (0.0, -1.3, 80.0, [1.0, 0.8, 0.1]),
        (1.4, -1.5, 10.0, [0.06, 0.06, 0.07]),
    ] {
        v.push(Part::cyl([x, 0.09, z], 0.18, 1.6, c).rot([90.0, yaw, 0.0]));
    }
    v
}

/// One cable head on the floor: a coil, a stalk and a plug with a red eye.
fn cable_head() -> Vec<Part> {
    let mut v = vec![
        Part::cyl([0.0, 0.08, 0.0], 0.8, 0.16, [0.06, 0.06, 0.07]),
        Part::cyl([0.0, 0.2, 0.0], 0.55, 0.12, [0.7, 0.1, 0.1]),
    ];
    v.extend(stalk([0.0, 0.2, 0.0], 1.0, 4, 0.32, 0.14, 30.0));
    v
}

/// The Exit Barrier of Car Park P3: a striped post with a ticket slot, a red eye and a long
/// arm held across your way out.
fn parking_barrier() -> Vec<Part> {
    const STRIPE: [f32; 3] = [0.95, 0.75, 0.05];
    let mut v = vec![
        Part::cube([0.0, 0.6, 0.0], [0.7, 1.2, 0.6], STRIPE),
        Part::cube([0.0, 1.25, 0.0], [0.75, 0.1, 0.65], DARK),
        Part::cube([0.0, 0.9, -0.31], [0.4, 0.08, 0.02], DARK),
        Part::ball([0.0, 1.45, -0.2], 0.22, [1.0, 0.1, 0.08]).glow(3.0),
    ];
    for k in 0..5 {
        let color = if k % 2 == 0 {
            [0.95, 0.95, 0.95]
        } else {
            [0.9, 0.12, 0.1]
        };
        v.push(Part::cube(
            [0.5 + k as f32 * 0.6, 1.05, 0.0],
            [0.6, 0.12, 0.12],
            color,
        ));
    }
    v
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

/// Boss of The Migration: the whole go-live weekend as one rack. A wall of red and green status
/// LEDs (a green field with red alarms among it, never the same two nights running), a glowing
/// "GO-LIVE" strip for a mouth, a siren on top and thick cables for arms, with more of them
/// pooled on the floor. Reads as a different monster from the hot-orange Cooling Failure.
fn cutover_rack() -> Vec<Part> {
    const GREEN: [f32; 3] = [0.1, 1.0, 0.3];
    const RED: [f32; 3] = [1.0, 0.08, 0.08];
    let mut v = vec![
        Part::cube([0.0, 1.85, 0.0], [2.2, 3.3, 1.3], [0.09, 0.1, 0.13]),
        Part::cube([0.0, 1.8, -0.67], [2.0, 3.0, 0.04], [0.16, 0.17, 0.21]),
        Part::cube([0.0, 0.12, 0.0], [2.4, 0.24, 1.5], DARK),
        // Eyes: two red slits under a dark brow.
        Part::cube([0.0, 3.3, -0.68], [1.8, 0.1, 0.05], DARK),
        Part::cube([-0.5, 3.05, -0.7], [0.5, 0.18, 0.03], RED).glow(3.0),
        Part::cube([0.5, 3.05, -0.7], [0.5, 0.18, 0.03], RED).glow(3.0),
        // Mouth: the go-live strip, with dark gaps for teeth.
        Part::cube([0.0, 0.62, -0.7], [1.6, 0.3, 0.03], RED).glow(2.5),
        // Siren on the roof.
        Part::cyl([0.0, 3.58, 0.0], 0.5, 0.18, DARK),
        Part::cyl([0.0, 3.78, 0.0], 0.36, 0.24, RED).glow(3.0),
        Part::cube([0.0, 3.78, 0.0], [0.0, 0.0, 0.0], RED)
            .with(vec![
                Part::cube([0.0, 0.0, -0.3], [0.1, 0.1, 0.6], [1.0, 0.7, 0.6]).glow(4.0),
            ])
            .spin(Vec3::Y, 5.0),
    ];
    for k in 0..4 {
        v.push(Part::cube(
            [-0.45 + k as f32 * 0.3, 0.62, -0.72],
            [0.06, 0.34, 0.02],
            DARK,
        ));
    }
    // The LED wall: mostly green, red where something is wrong, a few dark ones for dead hosts.
    for row in 0..8 {
        for col in 0..5 {
            let n = row * 5 + col;
            let (c, g) = match (n * 7 + row * 3) % 9 {
                0 | 4 => (RED, 2.2),
                6 => (DARK, 0.0),
                _ => (GREEN, 2.0),
            };
            let mut led = Part::cube(
                [-0.72 + col as f32 * 0.36, 1.05 + row as f32 * 0.24, -0.7],
                [0.2, 0.1, 0.02],
                c,
            )
            .glow(g);
            // Flicker: the red ones twitch, the way status lights do during a cutover.
            if c == RED {
                led = led.bob(0.015, 7.0 + n as f32);
            }
            v.push(led);
        }
    }
    // Cable arms: a black hawser, a red and a yellow one wrapped round it, a plug for a fist.
    for side in [-1.0_f32, 1.0] {
        v.push(
            Part::cyl([side * 1.4, 2.3, -0.2], 0.3, 1.9, [0.06, 0.06, 0.07]).rot([
                0.0,
                0.0,
                side * 14.0,
            ]),
        );
        v.push(
            Part::cyl([side * 1.52, 2.3, -0.2], 0.1, 1.9, RED)
                .glow(0.8)
                .rot([0.0, 0.0, side * 14.0]),
        );
        v.push(
            Part::cyl([side * 1.28, 2.3, -0.2], 0.08, 1.9, [1.0, 0.8, 0.1])
                .glow(0.8)
                .rot([0.0, 0.0, side * 14.0]),
        );
        v.push(Part::cube(
            [side * 1.78, 1.2, -0.3],
            [0.5, 0.45, 0.4],
            STEEL,
        ));
        v.push(Part::cube([side * 1.78, 0.9, -0.3], [0.3, 0.22, 0.3], GREEN).glow(2.0));
    }
    // Cables pooled on the floor in front of it.
    for (x, z, yaw, c) in [
        (-0.8, -1.2, 20.0, [0.06, 0.06, 0.07]),
        (0.5, -1.4, -30.0, RED),
        (0.0, -1.0, 80.0, [1.0, 0.8, 0.1]),
    ] {
        v.push(Part::cyl([x, 0.08, z], 0.16, 1.4, c).rot([90.0, yaw, 0.0]));
    }
    v
}

/// Boss of the QBR theatre: a golem of slides. Legs are stacks of printouts, the torso is one
/// huge slide with a blue title bar and a bar chart whose last bar is the only one that matters,
/// more slides fan out behind it, a projector for a head and two slide-board arms ending in
/// bullet points. A red laser dot wobbles over its chest.
fn slide_golem() -> Vec<Part> {
    const PAPER: [f32; 3] = [0.95, 0.95, 0.92];
    const BLUE: [f32; 3] = [0.15, 0.3, 0.72];
    const ORANGE: [f32; 3] = [0.98, 0.5, 0.1];
    const RED: [f32; 3] = [1.0, 0.1, 0.1];
    let mut v = vec![
        // Legs: two stacks of printouts, each sheet a little crooked.
        Part::cube([-0.6, 0.55, 0.0], [0.95, 1.1, 0.75], PAPER),
        Part::cube([0.6, 0.55, 0.0], [0.95, 1.1, 0.75], PAPER),
        Part::cube([-0.62, 0.9, -0.02], [1.0, 0.05, 0.8], [0.8, 0.85, 0.95]).rot([0.0, 8.0, 0.0]),
        Part::cube([0.62, 0.5, -0.02], [1.0, 0.05, 0.8], [0.95, 0.85, 0.8]).rot([0.0, -7.0, 0.0]),
        // Slides fanned out behind the torso.
        Part::cube([-0.25, 2.3, 0.4], [2.3, 2.1, 0.1], [0.78, 0.83, 0.95]).rot([0.0, 0.0, 5.0]),
        Part::cube([0.25, 2.3, 0.3], [2.3, 2.1, 0.1], [0.95, 0.88, 0.8]).rot([0.0, 0.0, -4.0]),
        // The torso: one very large slide with a title bar and a chart.
        Part::cube([0.0, 2.25, 0.0], [2.4, 2.2, 0.3], PAPER),
        Part::cube([0.0, 3.05, -0.17], [2.1, 0.4, 0.03], BLUE),
        Part::cube([0.0, 1.45, -0.17], [2.1, 0.04, 0.03], DARK),
        // Projector head: a dark box with a glowing lens and a vent of bars.
        Part::cube([0.0, 3.8, 0.0], [1.0, 0.5, 0.7], DARK),
        Part::cyl([0.0, 3.8, -0.42], 0.38, 0.2, [0.6, 0.9, 1.0])
            .glow(3.5)
            .rot([90.0, 0.0, 0.0]),
        Part::cube([0.0, 4.1, 0.0], [0.7, 0.06, 0.5], STEEL),
        // The laser dot that never leaves its chest.
        Part::ball([0.5, 2.3, -0.2], 0.12, RED)
            .glow(5.0)
            .bob(0.25, 2.3),
    ];
    // Title lines: three grey bars of "text" and a bullet each.
    for k in 0..3 {
        let y = 2.7 - k as f32 * 0.22;
        v.push(Part::ball([-0.95, y, -0.18], 0.1, DARK));
        v.push(Part::cube(
            [-0.15 - k as f32 * 0.1, y, -0.17],
            [1.5 - k as f32 * 0.2, 0.06, 0.02],
            [0.4, 0.42, 0.5],
        ));
    }
    // The hockey stick: flat bars, then straight up.
    for i in 0..6 {
        let h = 0.1 + (i as f32 / 5.0).powi(3) * 0.8;
        v.push(Part::cube(
            [-0.8 + i as f32 * 0.32, 1.47 + h / 2.0, -0.18],
            [0.22, h, 0.03],
            if i == 5 { ORANGE } else { BLUE },
        ));
    }
    // Arms: slide boards hanging from the shoulders, a big bullet point for a fist.
    for side in [-1.0_f32, 1.0] {
        v.push(
            Part::cube([side * 1.6, 2.4, -0.1], [0.12, 1.7, 0.85], PAPER).rot([
                0.0,
                0.0,
                side * 10.0,
            ]),
        );
        v.push(
            Part::cube([side * 1.6, 2.4, -0.16], [0.14, 1.5, 0.03], BLUE).rot([
                0.0,
                0.0,
                side * 10.0,
            ]),
        );
        v.push(Part::ball([side * 1.82, 1.3, -0.15], 0.5, DARK));
    }
    v
}

/// Trash: a KPI dashboard on a stand with wheels, a screen that is green all over, a chart of
/// bars that only go up, and one tiny red light it hopes you have not noticed.
fn kpi_dashboard() -> Vec<Part> {
    const GREEN: [f32; 3] = [0.1, 1.0, 0.3];
    let mut v = vec![
        Part::cube([0.0, 0.08, 0.0], [0.9, 0.16, 0.6], DARK),
        Part::cyl([0.0, 0.5, 0.0], 0.14, 0.8, STEEL),
        Part::cube([0.0, 1.35, 0.0], [1.5, 0.95, 0.12], DARK),
        Part::cube([0.0, 1.35, -0.07], [1.38, 0.82, 0.02], [0.03, 0.18, 0.08]).glow(1.2),
        Part::ball([0.58, 1.68, -0.09], 0.07, [1.0, 0.1, 0.1])
            .glow(4.0)
            .bob(0.01, 9.0),
        Part::cube([0.0, 0.2, -0.3], [0.8, 0.05, 0.02], GREEN).glow(2.0),
    ];
    for i in 0..7 {
        let h = 0.12 + i as f32 * 0.09;
        v.push(
            Part::cube(
                [-0.48 + i as f32 * 0.16, 0.98 + h / 2.0, -0.09],
                [0.1, h, 0.02],
                GREEN,
            )
            .glow(2.2),
        );
    }
    for sx in [-0.4, 0.4] {
        v.push(Part::cyl([sx, 0.06, 0.0], 0.14, 0.1, DARK).rot([0.0, 0.0, 90.0]));
    }
    v
}

/// Boss of the Hackathon's registration room: a tower of pizza boxes in a delivery cap. The
/// head is one huge pepperoni pizza with olive eyes and a dripping cheese grin, oven mitts hold a
/// pizza cutter the size of a wheel, and the belly is an oven door with a fire in it.
fn pizza_overlord() -> Vec<Part> {
    const BOX: [f32; 3] = [0.8, 0.62, 0.4];
    const CHEESE: [f32; 3] = [1.0, 0.8, 0.28];
    const RED: [f32; 3] = [0.78, 0.1, 0.08];
    const GREEN: [f32; 3] = [0.1, 0.3, 0.12];
    let mut v = vec![
        // Legs: two short stacks of boxes.
        Part::cube([-0.6, 0.55, 0.0], [0.95, 1.1, 0.95], BOX),
        Part::cube([0.6, 0.55, 0.0], [0.95, 1.1, 0.95], BOX),
        // Torso: a leaning stack, every box a little off, with a red band round the middle.
        Part::cube([0.0, 1.5, 0.0], [2.0, 0.5, 2.0], BOX),
        Part::cube([0.05, 1.98, 0.02], [2.0, 0.5, 2.0], [0.74, 0.58, 0.36]).rot([0.0, 6.0, 0.0]),
        Part::cube([-0.04, 2.46, 0.0], [2.0, 0.5, 2.0], BOX).rot([0.0, -5.0, 0.0]),
        Part::cube([0.0, 2.2, -1.02], [2.04, 0.12, 0.04], RED),
        // The belly oven: a black door and the fire behind its window.
        Part::cube([0.0, 1.85, -1.03], [1.1, 0.8, 0.05], DARK),
        Part::cube([0.0, 1.85, -1.07], [0.8, 0.5, 0.03], HOT).glow(3.5),
        // Head: a huge pizza turned to face front, with olives and a cheese grin.
        Part::cyl([0.0, 3.45, -0.15], 1.7, 0.22, CHEESE).rot([90.0, 0.0, 0.0]),
        Part::cyl([0.0, 3.45, -0.27], 1.7, 0.04, RED).rot([90.0, 0.0, 0.0]),
        Part::ball([-0.4, 3.7, -0.3], 0.34, GREEN),
        Part::ball([0.4, 3.7, -0.3], 0.34, GREEN),
        Part::ball([-0.4, 3.7, -0.43], 0.14, [1.0, 0.1, 0.1]).glow(4.0),
        Part::ball([0.4, 3.7, -0.43], 0.14, [1.0, 0.1, 0.1]).glow(4.0),
        Part::cube([0.0, 3.12, -0.3], [0.9, 0.2, 0.06], DARK),
        // The delivery cap: red with a visor and a logo that is mostly a smile.
        Part::cyl([0.0, 4.35, -0.1], 1.3, 0.3, RED),
        Part::cube([0.0, 4.2, -0.65], [1.3, 0.06, 0.55], RED),
        // Cheese drips from the chin and the arms.
        Part::cube([-0.5, 2.85, -0.3], [0.12, 0.5, 0.04], CHEESE),
        Part::cube([0.1, 2.9, -0.3], [0.12, 0.35, 0.04], CHEESE),
        Part::cube([0.55, 2.8, -0.3], [0.12, 0.6, 0.04], CHEESE),
    ];
    // Pepperoni on the head.
    for (x, y) in [(-0.1, 3.9), (0.6, 3.1), (-0.65, 3.2), (0.1, 3.3)] {
        v.push(Part::cyl([x, y, -0.3], 0.26, 0.03, RED).rot([90.0, 0.0, 0.0]));
    }
    // Arms: sleeves and oven mitts, one holding the cutter.
    for side in [-1.0_f32, 1.0] {
        v.push(
            Part::cyl([side * 1.35, 2.3, -0.1], 0.35, 1.3, [0.9, 0.9, 0.88]).rot([
                0.0,
                0.0,
                side * 14.0,
            ]),
        );
        v.push(Part::ball([side * 1.55, 1.5, -0.3], 0.62, [0.3, 0.3, 0.38]));
    }
    v.push(
        Part::cyl([1.55, 1.5, -0.75], 1.2, 0.06, STEEL)
            .rot([0.0, 0.0, 90.0])
            .spin(Vec3::X, 2.0),
    );
    v.push(Part::cube([1.55, 1.5, -0.3], [0.1, 0.1, 0.9], DARK));
    v
}

/// Boss of the Demo Stage: a gremlin in a lanyard, huge ears and a wide white grin, with a
/// laptop on its back that is quite on fire and shows a blue screen.
fn demo_gremlin() -> Vec<Part> {
    const SKIN: [f32; 3] = [0.38, 0.62, 0.3];
    const BSOD: [f32; 3] = [0.1, 0.25, 1.0];
    let mut v = vec![
        Part::ball([0.0, 1.1, 0.0], 1.5, SKIN),
        Part::ball([0.0, 2.1, -0.15], 1.2, SKIN),
        Part::cube([0.0, 1.0, -0.74], [0.3, 0.7, 0.04], [0.9, 0.9, 0.9]),
        Part::cube([0.0, 0.7, -0.76], [0.45, 0.35, 0.03], [0.9, 0.2, 0.9]),
        Part::cyl([-0.4, 0.3, 0.0], 0.4, 0.6, SKIN),
        Part::cyl([0.4, 0.3, 0.0], 0.4, 0.6, SKIN),
        Part::cube([-0.4, 0.04, -0.2], [0.5, 0.1, 0.7], DARK),
        Part::cube([0.4, 0.04, -0.2], [0.5, 0.1, 0.7], DARK),
        // Eyes and a mouth full of teeth.
        Part::ball([-0.3, 2.25, -0.65], 0.34, [1.0, 1.0, 0.6]),
        Part::ball([0.3, 2.25, -0.65], 0.34, [1.0, 1.0, 0.6]),
        Part::ball([-0.3, 2.25, -0.8], 0.13, [1.0, 0.05, 0.05]).glow(4.0),
        Part::ball([0.3, 2.25, -0.8], 0.13, [1.0, 0.05, 0.05]).glow(4.0),
        Part::cube([0.0, 1.85, -0.72], [0.7, 0.18, 0.04], DARK),
        // Ears: huge, flat and pointing out.
        Part::cube([-0.95, 2.45, -0.1], [0.9, 0.7, 0.06], SKIN).rot([0.0, 0.0, 28.0]),
        Part::cube([0.95, 2.45, -0.1], [0.9, 0.7, 0.06], SKIN).rot([0.0, 0.0, -28.0]),
        // The laptop on fire, strapped to the back.
        Part::cube([0.0, 1.5, 0.75], [1.4, 0.9, 0.08], DARK).rot([-12.0, 0.0, 0.0]),
        Part::cube([0.0, 1.5, 0.7], [1.25, 0.75, 0.03], BSOD)
            .glow(2.5)
            .rot([-12.0, 0.0, 0.0]),
        Part::ball([0.4, 2.15, 0.8], 0.35, HOT)
            .glow(5.0)
            .bob(0.1, 7.0),
        Part::ball([-0.2, 2.3, 0.8], 0.26, [1.0, 0.8, 0.1])
            .glow(5.0)
            .bob(0.12, 9.0),
    ];
    // Arms: long, thin, with claws; one holds a clicker.
    for side in [-1.0_f32, 1.0] {
        v.push(Part::cyl([side * 0.95, 1.1, -0.3], 0.2, 1.3, SKIN).rot([55.0, 0.0, side * 15.0]));
        v.push(Part::ball([side * 1.05, 0.65, -0.85], 0.3, SKIN));
    }
    v.push(Part::cube(
        [1.05, 0.7, -1.1],
        [0.1, 0.1, 0.4],
        [0.85, 0.1, 0.1],
    ));
    v
}

/// Trash: the energy drink pyramid, 47 cans in rows with a crushed one on top.
fn energy_pyramid() -> Vec<Part> {
    let mut v = Vec::new();
    for (k, n) in [5usize, 4, 2, 1].into_iter().enumerate() {
        for i in 0..n {
            for j in 0..n {
                let o = (n as f32 - 1.0) / 2.0;
                let shade = [
                    [0.15, 0.9, 0.3],
                    [0.2, 0.8, 1.0],
                    [1.0, 0.8, 0.1],
                    [1.0, 0.2, 0.7],
                ][(i + j + k) % 4];
                v.push(Part::cyl(
                    [
                        (i as f32 - o) * 0.27,
                        0.16 + k as f32 * 0.32,
                        (j as f32 - o) * 0.27,
                    ],
                    0.25,
                    0.3,
                    shade,
                ));
            }
        }
    }
    v.push(Part::cyl([0.0, 1.45, 0.0], 0.25, 0.14, STEEL));
    v.push(
        Part::ball([0.0, 1.0, -0.6], 0.1, [1.0, 0.1, 0.1])
            .glow(4.0)
            .bob(0.02, 12.0),
    );
    v
}

/// Add: a slice of pizza on little legs, cold, with one pepperoni for an eye.
fn pizza_slice() -> Vec<Part> {
    vec![
        Part::cube([0.0, 0.5, 0.0], [0.7, 0.1, 0.9], [1.0, 0.8, 0.3]).rot([0.0, 0.0, 0.0]),
        Part::cube([0.0, 0.46, 0.5], [0.7, 0.14, 0.12], [0.8, 0.55, 0.25]),
        Part::cube([0.0, 0.5, -0.5], [0.2, 0.1, 0.3], [1.0, 0.8, 0.3]),
        Part::ball([0.12, 0.58, -0.2], 0.2, [0.8, 0.1, 0.08]),
        Part::ball([-0.15, 0.58, 0.1], 0.18, [0.8, 0.1, 0.08]),
        Part::ball([0.1, 0.6, -0.34], 0.07, [1.0, 1.0, 1.0]),
        Part::cyl([-0.2, 0.2, 0.0], 0.12, 0.4, DARK),
        Part::cyl([0.2, 0.2, 0.0], 0.12, 0.4, DARK),
    ]
}

/// Add: Low-Hanging Fruit, an apple on little legs with a leaf and a very eager face.
fn apple() -> Vec<Part> {
    const RED: [f32; 3] = [0.82, 0.1, 0.1];
    const GREEN: [f32; 3] = [0.25, 0.65, 0.2];
    vec![
        Part::ball([0.0, 0.62, 0.0], 0.72, RED).bob(0.06, 5.0),
        Part::ball([0.0, 0.58, 0.0], 0.64, [0.9, 0.18, 0.12]),
        Part::cube([0.0, 1.05, 0.0], [0.06, 0.2, 0.06], [0.35, 0.22, 0.1]),
        Part::cube([0.14, 1.12, 0.0], [0.26, 0.04, 0.14], GREEN).rot([0.0, 0.0, 20.0]),
        Part::ball([-0.14, 0.7, -0.3], 0.13, [1.0, 1.0, 1.0]),
        Part::ball([0.14, 0.7, -0.3], 0.13, [1.0, 1.0, 1.0]),
        Part::cube([0.0, 0.5, -0.34], [0.3, 0.05, 0.03], DARK),
        Part::cyl([-0.18, 0.12, 0.0], 0.1, 0.25, DARK),
        Part::cyl([0.18, 0.12, 0.0], 0.1, 0.25, DARK),
    ]
}

/// Add: a Hallucination, a glowing teal blob with a very sure expression and a question mark.
fn hallucination() -> Vec<Part> {
    vec![
        Part::ball([0.0, 0.7, 0.0], 0.9, [0.3, 0.9, 0.85])
            .glow(1.6)
            .bob(0.12, 3.0),
        Part::ball([-0.15, 0.8, -0.38], 0.16, [1.0, 1.0, 1.0]),
        Part::ball([0.15, 0.8, -0.38], 0.16, [1.0, 1.0, 1.0]),
        Part::cube([0.0, 0.55, -0.42], [0.3, 0.05, 0.03], DARK),
        Part::cube([0.0, 1.35, 0.0], [0.08, 0.22, 0.08], MAGENTA)
            .glow(3.0)
            .bob(0.08, 4.0),
        Part::ball([0.0, 1.1, 0.0], 0.09, MAGENTA)
            .glow(3.0)
            .bob(0.08, 4.0),
    ]
}

/// Overlay for the Rollback Wraith: a spinning ring of light over its head, a glowing "v1.0"
/// tag hovering above that (letters are bars; the unit faces -Z so the text runs towards -X)
/// and a cold ring on the floor where its feet would be if it had been deployed properly.
fn release_halo() -> Vec<Part> {
    const ICE: [f32; 3] = [0.5, 0.85, 1.0];
    let mut v = vec![
        Part::cyl([0.0, 0.02, 0.0], 1.5, 0.02, ICE).glow(2.0),
        Part::cyl([0.0, 0.025, 0.0], 1.1, 0.02, DARK),
    ];
    let ring = |x: f32, z: f32, deg: f32| {
        Part::cube([x, 0.0, z], [0.28, 0.07, 0.07], ICE)
            .glow(3.0)
            .rot([0.0, deg, 0.0])
    };
    let mut halo = Vec::new();
    for k in 0..8 {
        let a = k as f32 * TAU / 8.0;
        halo.push(ring(0.5 * a.cos(), 0.5 * a.sin(), -a.to_degrees()));
    }
    v.push(
        Part::cube([0.0, 2.7, 0.0], [0.0, 0.0, 0.0], ICE)
            .with(halo)
            .spin(Vec3::Y, 1.2),
    );
    // "v1.0", read from the front: later glyphs sit at more negative x.
    let bar = |x: f32, y: f32, w: f32, h: f32, deg: f32| {
        Part::cube([x, y, -0.02], [w, h, 0.05], ICE)
            .glow(2.5)
            .rot([0.0, 0.0, deg])
            .bob(0.05, 1.6)
    };
    let y = 3.15;
    v.push(Part::cube([0.0, y, 0.03], [1.0, 0.5, 0.03], [0.1, 0.16, 0.24]).bob(0.05, 1.6));
    v.push(bar(0.34, y - 0.02, 0.07, 0.3, -20.0));
    v.push(bar(0.2, y - 0.02, 0.07, 0.3, 20.0));
    v.push(bar(0.0, y, 0.07, 0.34, 0.0));
    v.push(bar(0.07, y + 0.12, 0.07, 0.12, 40.0));
    v.push(bar(-0.14, y - 0.15, 0.06, 0.06, 0.0));
    v.push(bar(-0.3, y + 0.15, 0.22, 0.05, 0.0));
    v.push(bar(-0.3, y - 0.15, 0.22, 0.05, 0.0));
    v.push(bar(-0.2, y, 0.05, 0.3, 0.0));
    v.push(bar(-0.4, y, 0.05, 0.3, 0.0));
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

/// VibeCon's Unreviewed PR: a floating page of diff, green and red lines, an orange merge bar and
/// a fuse ring spinning round it while the merge counts down.
fn unreviewed_pr() -> Vec<Part> {
    const PAGE: [f32; 3] = [0.95, 0.95, 0.97];
    const ADD: [f32; 3] = [0.2, 0.95, 0.35];
    const DEL: [f32; 3] = [1.0, 0.3, 0.3];
    let mut page = vec![
        Part::cube([0.0, 0.0, 0.0], [0.9, 1.2, 0.05], PAGE),
        Part::cube([0.0, 0.5, -0.03], [0.8, 0.1, 0.01], [0.3, 0.32, 0.4]),
        Part::cube([0.0, -0.5, -0.03], [0.7, 0.1, 0.01], HOT).glow(3.0),
    ];
    for k in 0..7 {
        let (w, c) = if k % 3 == 2 {
            (0.4, DEL)
        } else {
            (0.6 - 0.05 * k as f32, ADD)
        };
        page.push(
            Part::cube(
                [-0.35 + w / 2.0, 0.32 - k as f32 * 0.1, -0.03],
                [w, 0.05, 0.01],
                c,
            )
            .glow(1.5),
        );
    }
    let ring = (0..8)
        .map(|k| {
            let a = k as f32 * TAU / 8.0;
            Part::cube(
                [0.75 * a.cos(), 0.0, 0.75 * a.sin()],
                [0.22, 0.06, 0.08],
                HOT,
            )
            .glow(2.5)
            .rot([0.0, -a.to_degrees() + 90.0, 0.0])
        })
        .collect();
    vec![
        Part::cube([0.0, 0.9, 0.0], [0.0; 3], PAGE)
            .with(page)
            .bob(0.08, 2.0),
        Part::cube([0.0, 0.9, 0.0], [0.0; 3], HOT)
            .with(ring)
            .spin(Vec3::Y, 2.5),
        Part::cyl([0.0, 0.02, 0.0], 0.9, 0.03, DARK),
    ]
}

/// Live Demo Bug: a knee-high beetle with glowing eyes, six legs and twitchy antennae.
fn demo_bug() -> Vec<Part> {
    const SHELL: [f32; 3] = [0.55, 0.08, 0.12];
    let mut v = vec![
        Part::ball([0.0, 0.3, 0.05], 0.55, SHELL),
        Part::cube([0.0, 0.42, 0.05], [0.02, 0.2, 0.5], DARK),
        Part::ball([0.0, 0.3, -0.3], 0.28, DARK),
        Part::ball([-0.07, 0.36, -0.42], 0.07, [1.0, 0.85, 0.1]).glow(3.0),
        Part::ball([0.07, 0.36, -0.42], 0.07, [1.0, 0.85, 0.1]).glow(3.0),
        Part::cube([-0.08, 0.5, -0.48], [0.02, 0.02, 0.25], DARK).rot([30.0, -20.0, 0.0]),
        Part::cube([0.08, 0.5, -0.48], [0.02, 0.02, 0.25], DARK).rot([30.0, 20.0, 0.0]),
    ];
    for s in [-1.0, 1.0] {
        for k in 0..3 {
            v.push(
                Part::cube(
                    [0.28 * s, 0.14, -0.12 + k as f32 * 0.17],
                    [0.3, 0.03, 0.03],
                    DARK,
                )
                .rot([0.0, 0.0, -25.0 * s]),
            );
        }
    }
    v
}

/// AGI (Beta): a hovering glow orb wearing an orange BETA band, two counter-rotating rings.
fn agi_beta() -> Vec<Part> {
    const ORANGE: [f32; 3] = [1.0, 0.55, 0.1];
    let ring = |r: f32| -> Vec<Part> {
        (0..10)
            .map(|k| {
                let a = k as f32 * TAU / 10.0;
                Part::cube([r * a.cos(), 0.0, r * a.sin()], [0.35, 0.06, 0.12], CYAN)
                    .glow(3.0)
                    .rot([0.0, -a.to_degrees() + 90.0, 0.0])
            })
            .collect()
    };
    vec![
        Part::cube([0.0, 1.3, 0.0], [0.0; 3], CYAN)
            .with(vec![
                Part::ball([0.0, 0.0, 0.0], 1.0, [0.75, 0.95, 1.0]).glow(1.5),
                Part::cyl([0.0, 0.0, 0.0], 1.04, 0.16, ORANGE).glow(2.0),
                Part::cube([-0.15, 0.12, -0.46], [0.12, 0.05, 0.02], DARK),
                Part::cube([0.15, 0.12, -0.46], [0.12, 0.05, 0.02], DARK),
            ])
            .bob(0.12, 1.4),
        Part::cube([0.0, 1.3, 0.0], [0.0; 3], CYAN)
            .with(ring(0.95))
            .spin(Vec3::Y, 1.2),
        Part::cube([0.0, 1.3, 0.0], [0.0; 3], CYAN)
            .with(ring(1.15))
            .spin(Vec3::Y, -0.8),
        Part::cyl([0.0, 0.03, 0.0], 1.2, 0.04, DARK),
    ]
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
        "wifi_router" => {
            // Four antennas, one orange LED (never green), and a "connecting" spinner above.
            let mut v = vec![
                Part::cube([0.0, 0.1, 0.0], [0.42, 0.09, 0.3], [0.92, 0.92, 0.95]),
                Part::cube([0.0, 0.05, 0.0], [0.44, 0.02, 0.32], DARK),
                Part::cube([-0.12, 0.15, -0.151], [0.04, 0.02, 0.01], ORANGE).glow(3.5),
                Part::cube([0.0, 0.15, -0.151], [0.04, 0.02, 0.01], DARK),
                Part::cube([0.12, 0.15, -0.151], [0.04, 0.02, 0.01], DARK),
            ];
            for (k, x) in [-0.18, -0.06, 0.06, 0.18].into_iter().enumerate() {
                let tilt = [-14.0, -5.0, 5.0, 14.0][k];
                v.push(
                    Part::cube([x, 0.3, 0.12], [0.025, 0.32, 0.025], DARK).rot([0.0, 0.0, tilt]),
                );
            }
            let dots = (0..6)
                .map(|k| {
                    let a = k as f32 * TAU / 6.0;
                    Part::ball([0.14 * a.cos(), 0.0, 0.14 * a.sin()], 0.04, ORANGE).glow(2.5)
                })
                .collect();
            v.push(
                Part::cube([0.0, 0.62, 0.0], [0.0; 3], ORANGE)
                    .with(dots)
                    .spin(Vec3::Y, 3.0),
            );
            v
        }
        "party_leftovers" => {
            // A takeaway tub with its foil lid half open, a skewer poking out and a curl of
            // steam: still technically food.
            let mut v = vec![
                Part::cube([0.0, 0.12, 0.0], [0.42, 0.22, 0.3], [0.95, 0.95, 0.92]),
                Part::cube([0.0, 0.235, 0.0], [0.44, 0.02, 0.32], [0.75, 0.78, 0.82])
                    .rot([-18.0, 0.0, 0.0]),
                Part::ball([-0.1, 0.25, 0.04], 0.1, [0.95, 0.75, 0.3]),
                Part::ball([0.06, 0.25, -0.02], 0.09, [0.85, 0.3, 0.25]),
                Part::cube([0.12, 0.34, 0.04], [0.015, 0.3, 0.015], [0.7, 0.55, 0.35])
                    .rot([0.0, 0.0, -25.0]),
                Part::ball([-0.07, 0.13, -0.151], 0.05, DARK),
                Part::ball([0.07, 0.13, -0.151], 0.05, DARK),
            ];
            let steam = (0..3)
                .map(|k| {
                    let y = k as f32 * 0.08;
                    Part::ball([0.03 * (k as f32 - 1.0), y, 0.0], 0.07, [0.95, 0.95, 1.0]).glow(0.6)
                })
                .collect();
            v.push(
                Part::cube([0.0, 0.42, 0.0], [0.0; 3], [0.95, 0.95, 1.0])
                    .with(steam)
                    .bob(0.04, 1.6),
            );
            v
        }
        _ => vec![Part::ball([0.0, 0.2, 0.0], 0.3, MAGENTA)],
    }
}

/// Kick scooter that goes under a character's feet.
/// Survey quadcopter hovering at head height: a body, four arms with spinning rotors, a red
/// laser eye.
fn drone() -> Vec<Part> {
    let mut v = vec![
        Part::cube([0.0, 1.6, 0.0], [0.4, 0.14, 0.5], [0.92, 0.92, 0.9]).bob(0.08, 2.5),
        Part::ball([0.0, 1.5, -0.24], 0.14, DARK).bob(0.08, 2.5),
        Part::ball([0.0, 1.5, -0.3], 0.06, [1.0, 0.1, 0.1])
            .glow(3.0)
            .bob(0.08, 2.5),
    ];
    for (x, z) in [(-0.38, -0.38), (0.38, -0.38), (-0.38, 0.38), (0.38, 0.38)] {
        v.push(
            Part::cube([x / 2.0, 1.62, z / 2.0], [0.5, 0.04, 0.05], STEEL)
                .rot([0.0, 45.0, 0.0])
                .bob(0.08, 2.5),
        );
        v.push(
            Part::cyl([x, 1.68, z], 0.42, 0.015, [0.3, 0.32, 0.36])
                .spin(Vec3::Y, 30.0)
                .bob(0.08, 2.5),
        );
    }
    v
}

/// The Campus Goose: white body, long black neck, orange beak and legs. Smug.
fn goose() -> Vec<Part> {
    const WHITE: [f32; 3] = [0.94, 0.93, 0.9];
    const ORANGE: [f32; 3] = [1.0, 0.55, 0.1];
    vec![
        Part::ball([0.0, 0.75, 0.15], 0.9, WHITE),
        Part::ball([0.0, 0.85, 0.55], 0.45, [0.6, 0.58, 0.55]),
        Part::cyl([0.0, 1.3, -0.25], 0.16, 0.8, DARK).rot([-20.0, 0.0, 0.0]),
        Part::ball([0.0, 1.72, -0.4], 0.28, DARK),
        Part::cube([0.0, 1.68, -0.62], [0.1, 0.07, 0.24], ORANGE),
        Part::ball([-0.1, 1.77, -0.48], 0.06, WHITE),
        Part::ball([0.1, 1.77, -0.48], 0.06, WHITE),
        Part::cyl([-0.15, 0.2, 0.15], 0.06, 0.4, ORANGE),
        Part::cyl([0.15, 0.2, 0.15], 0.06, 0.4, ORANGE),
        Part::cube([-0.15, 0.02, 0.05], [0.18, 0.03, 0.22], ORANGE),
        Part::cube([0.15, 0.02, 0.05], [0.18, 0.03, 0.22], ORANGE),
    ]
}

/// Mochi, the home office cat: a ginger loaf with a raised tail. Faces -Z like every unit.
fn cat() -> Vec<Part> {
    const GINGER: [f32; 3] = [0.9, 0.55, 0.22];
    const CREAM: [f32; 3] = [0.98, 0.9, 0.78];
    const PINK: [f32; 3] = [1.0, 0.6, 0.65];
    let mut v = vec![
        Part::new(Shape::Sphere, [0.0, 0.3, 0.05], [0.42, 0.36, 0.75], GINGER),
        Part::ball([0.0, 0.3, -0.24], 0.3, CREAM),
        Part::ball([0.0, 0.55, -0.36], 0.36, GINGER),
        Part::ball([0.0, 0.5, -0.52], 0.14, CREAM),
        Part::ball([0.0, 0.53, -0.56], 0.04, PINK),
        Part::ball([-0.08, 0.6, -0.52], 0.06, [0.4, 0.9, 0.35]).glow(0.6),
        Part::ball([0.08, 0.6, -0.52], 0.06, [0.4, 0.9, 0.35]).glow(0.6),
        Part::cyl([0.0, 0.5, 0.5], 0.07, 0.55, GINGER).rot([25.0, 0.0, 0.0]),
    ];
    for x in [-0.11, 0.11] {
        v.push(
            Part::cube([x, 0.75, -0.36], [0.09, 0.13, 0.04], GINGER).rot([0.0, 0.0, -x * 200.0]),
        );
    }
    for (x, z) in [(-0.12, -0.2), (0.12, -0.2), (-0.12, 0.28), (0.12, 0.28)] {
        v.push(Part::cyl([x, 0.08, z], 0.09, 0.16, CREAM));
    }
    v
}

/// The free pizza: three open boxes on a folding table.
fn pizza() -> Vec<Part> {
    const BOX: [f32; 3] = [0.85, 0.68, 0.45];
    let mut v = vec![Part::cube(
        [0.0, 0.85, 0.0],
        [1.6, 0.06, 0.8],
        [0.95, 0.95, 0.95],
    )];
    for (x, z) in [(-0.6, 0.0), (0.6, 0.0)] {
        v.push(Part::cube([x, 0.42, z], [0.05, 0.84, 0.7], STEEL));
    }
    for (k, x) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
        v.push(Part::cube([x, 0.9, 0.0], [0.44, 0.04, 0.44], BOX));
        v.push(Part::cyl([x, 0.93, 0.0], 0.38, 0.02, [0.95, 0.75, 0.3]));
        v.push(Part::cube([x, 1.12, 0.21], [0.44, 0.44, 0.03], BOX).rot([-15.0, 0.0, 0.0]));
        if k != 1 {
            v.push(Part::ball([x + 0.08, 0.95, -0.05], 0.07, [0.8, 0.15, 0.1]));
        }
    }
    v
}

/// The last working espresso cart: a box on wheels, a chrome machine, steam that never stops.
fn espresso_cart() -> Vec<Part> {
    let mut v = vec![
        Part::cube([0.0, 0.55, 0.0], [1.2, 0.7, 0.7], [0.45, 0.2, 0.12]),
        Part::cube([0.0, 0.92, 0.0], [1.3, 0.05, 0.8], [0.7, 0.7, 0.72]),
        Part::cube([0.1, 1.2, 0.05], [0.6, 0.5, 0.45], [0.8, 0.82, 0.86]),
        Part::cube([0.1, 1.25, -0.18], [0.3, 0.12, 0.02], HOT).glow(2.0),
        Part::ball([0.1, 1.6, 0.05], 0.18, [0.95, 0.95, 0.95]).bob(0.1, 3.0),
        Part::cyl([-0.4, 1.1, 0.0], 0.14, 0.3, [0.9, 0.9, 0.9]),
    ];
    for x in [-0.45, 0.45] {
        v.push(Part::cyl([x, 0.16, 0.0], 0.32, 0.1, DARK).rot([0.0, 0.0, 90.0]));
    }
    v
}

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
    /// One lit material per screen picture.
    screens: HashMap<&'static str, Handle<StandardMaterial>>,
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
    assets: &mut RigAssets,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    if let Role::Screen(picture) = part.role {
        let mat = assets
            .screens
            .entry(picture)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color_texture: crate::floors::rig_screen(picture).map(|i| images.add(i)),
                    base_color: Color::WHITE,
                    emissive: LinearRgba::rgb(0.9, 0.9, 0.9),
                    emissive_texture: None,
                    unlit: true,
                    ..default()
                })
            })
            .clone();
        let tf =
            Transform::from_translation(Vec3::from(part.pos)).with_scale(Vec3::from(part.size));
        let mut e = parent.spawn((Mesh3d(shapes[0].clone()), MeshMaterial3d(mat), tf));
        e.insert(NotShadowCaster);
        return;
    }
    let mats = &mut assets.mats;
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
        Role::Plain | Role::Screen(_) => {}
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
                spawn_part(c, child, shapes, assets, materials, images);
            }
        });
    }
}

fn build_rigs(
    mut commands: Commands,
    mut assets: ResMut<RigAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
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
                spawn_part(body, part, &shapes, assets, &mut materials, &mut images);
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
        let state = RigState {
            body,
            shelled,
            open: 0.0,
            reveal: false,
        };
        // The rig's owner can be despawned before these commands apply (a scene swapped in the
        // same frame); then the freshly built body goes too instead of erroring and leaking.
        commands.queue(move |world: &mut World| {
            if let Ok(mut owner) = world.get_entity_mut(entity) {
                owner.add_child(body).insert(state);
            } else if let Ok(orphan) = world.get_entity_mut(body) {
                orphan.despawn();
            }
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
            "cutover_rack",
            "tentacle_rack",
            "patch_tuesday",
            "perf_review",
            "hr_bot",
            "stakeholder",
            "cable_hydra",
            "cable_head",
            "slide_golem",
            "kpi_dashboard",
            "pizza_overlord",
            "demo_gremlin",
            "energy_pyramid",
            "pizza_slice",
            "apple",
            "hallucination",
            "release_halo",
            "agi_prototype",
            "scooter",
            "unreviewed_pr",
            "demo_bug",
            "agi_beta",
            "pet_wifi_router",
            "pet_party_leftovers",
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
