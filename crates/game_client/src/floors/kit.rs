//! Pure floor-plan vocabulary: pieces, materials, textures and the `Plan`
//! builder. Nothing here touches the ECS or assets, so layouts stay
//! deterministic and unit-testable; `mod.rs` interprets the pieces.

// The furniture and texture catalogue is larger than what the five floors place today.
#![allow(dead_code)]

use super::shape::{Footprint, Seg, WALL_T as COLLIDER_T, colliders};
use crate::collision::aabb;
use crate::config::WALL_HEIGHT;
use bevy::prelude::*;
use combat_core::Aabb;
use std::f32::consts::{FRAC_PI_2, PI};

/// Kenney furniture is modelled ~2.4x too small for a 1.8 m character.
pub const FURNITURE_SCALE: f32 = 2.4;

/// Furniture GLB plus the bounding box measured from its glTF accessors
/// (model space, metres). Kit origins sit at a corner of the box or its
/// centre depending on the piece, so placement centres the footprint.
/// The model front is +Z.
#[derive(Debug)]
pub struct Model {
    pub file: &'static str,
    /// Minimum x and z of the box.
    pub lo: [f32; 2],
    /// Width (x), height (y) and depth (z).
    pub size: [f32; 3],
}

impl Model {
    const fn new(file: &'static str, lo: [f32; 2], size: [f32; 3]) -> Self {
        Self { file, lo, size }
    }
}

pub static BOOKCASE_CLOSED_WIDE: Model =
    Model::new("bookcaseClosedWide", [0.0, -0.25], [0.8, 0.79, 0.25]);
pub static BOOKCASE_CLOSED: Model = Model::new("bookcaseClosed", [0.0, -0.25], [0.4, 0.85, 0.25]);
pub static BOOKCASE_OPEN: Model = Model::new("bookcaseOpen", [0.0, -0.25], [0.4, 0.88, 0.25]);
pub static BOOKS: Model = Model::new("books", [0.0, -0.095], [0.15, 0.104, 0.095]);
pub static CHAIR_DESK: Model = Model::new("chairDesk", [0.0, -0.314], [0.335, 0.608, 0.314]);
pub static CHAIR_MODERN: Model = Model::new("chairModernCushion", [0.0, -0.2], [0.2, 0.46, 0.2]);
pub static COAT_RACK: Model =
    Model::new("coatRackStanding", [-0.136, -0.136], [0.273, 0.77, 0.273]);
pub static KEYBOARD: Model = Model::new("computerKeyboard", [0.0, -0.118], [0.282, 0.028, 0.118]);
pub static MOUSE: Model = Model::new("computerMouse", [-0.025, -0.085], [0.05, 0.024, 0.085]);
pub static SCREEN: Model = Model::new("computerScreen", [0.0, -0.104], [0.393, 0.294, 0.104]);
pub static DESK: Model = Model::new("desk", [-0.01, -0.38], [0.734, 0.384, 0.392]);
pub static COFFEE_MACHINE: Model =
    Model::new("kitchenCoffeeMachine", [0.0, -0.24], [0.19, 0.177, 0.24]);
pub static FRIDGE: Model = Model::new("kitchenFridge", [0.0, -0.282], [0.43, 0.92, 0.292]);
pub static LAMP_TABLE: Model =
    Model::new("lampRoundTable", [-0.016, -0.148], [0.152, 0.314, 0.176]);
pub static LAMP_FLOOR: Model = Model::new("lampSquareFloor", [0.0, -0.12], [0.12, 0.86, 0.12]);
pub static LAMP_ROUND_FLOOR: Model =
    Model::new("lampRoundFloor", [-0.016, -0.148], [0.152, 0.86, 0.176]);
pub static LAPTOP: Model = Model::new("laptop", [0.0, -0.24], [0.264, 0.162, 0.24]);
pub static LOUNGE_CHAIR: Model = Model::new("loungeChair", [0.0, -0.41], [0.49, 0.46, 0.41]);
pub static LOUNGE_CHAIR_RELAX: Model =
    Model::new("loungeChairRelax", [0.0, -0.675], [0.49, 0.63, 0.675]);
pub static LOUNGE_SOFA: Model = Model::new("loungeSofa", [0.0, -0.41], [0.98, 0.46, 0.41]);
pub static LOUNGE_SOFA_LONG: Model = Model::new("loungeSofaLong", [0.0, -0.41], [0.98, 0.46, 0.82]);
pub static LOUNGE_DESIGN_SOFA: Model =
    Model::new("loungeDesignSofa", [0.0, -0.41], [1.12, 0.4, 0.41]);
pub static PLANTS_SMALL: [Model; 3] = [
    Model::new("plantSmall1", [-0.047, -0.047], [0.095, 0.14, 0.095]),
    Model::new("plantSmall2", [-0.047, -0.047], [0.095, 0.14, 0.095]),
    Model::new("plantSmall3", [-0.049, -0.042], [0.098, 0.145, 0.085]),
];
pub static POTTED_PLANT: Model = Model::new("pottedPlant", [-0.106, -0.121], [0.212, 0.654, 0.241]);
pub static RUG: Model = Model::new("rugRectangle", [0.0, -0.92], [1.57, 0.01, 0.92]);
pub static RUG_ROUND: Model = Model::new("rugRound", [0.0, -0.92], [0.92, 0.01, 0.92]);
pub static SIDE_TABLE: Model = Model::new("sideTable", [-0.01, -0.21], [0.534, 0.384, 0.22]);
pub static COFFEE_TABLE: Model = Model::new("tableCoffee", [-0.461, -0.3], [0.661, 0.23, 0.4]);
pub static COFFEE_TABLE_GLASS: Model =
    Model::new("tableCoffeeGlass", [-0.461, -0.3], [0.661, 0.23, 0.4]);
pub static TABLE: Model = Model::new("table", [0.0, -0.447], [0.841, 0.327, 0.447]);
pub static TABLE_GLASS: Model = Model::new("tableGlass", [0.0, -0.447], [0.841, 0.327, 0.447]);
pub static TABLE_ROUND: Model = Model::new("tableRound", [0.0, -0.8], [0.693, 0.367, 0.8]);
pub static TV: Model = Model::new("televisionModern", [-0.342, -0.064], [0.685, 0.455, 0.128]);
pub static CABINET_TV: Model = Model::new("cabinetTelevision", [0.0, -0.25], [0.8, 0.31, 0.25]);
pub static STOOL_BAR: Model = Model::new("stoolBar", [0.0, -0.23], [0.265, 0.435, 0.23]);
pub static BOX_CLOSED: Model =
    Model::new("cardboardBoxClosed", [0.0, -0.212], [0.212, 0.281, 0.212]);
pub static BENCH_CUSHION: Model = Model::new("benchCushion", [0.0, -0.2], [0.4, 0.46, 0.2]);
pub static TRASHCAN: Model = Model::new("trashcan", [-0.104, -0.112], [0.208, 0.428, 0.234]);

/// Every furniture file the floors reference, for the asset-existence test.
pub static ALL_MODELS: [&Model; 41] = [
    &BOOKCASE_CLOSED_WIDE,
    &BOOKCASE_CLOSED,
    &BOOKCASE_OPEN,
    &BOOKS,
    &CHAIR_DESK,
    &CHAIR_MODERN,
    &COAT_RACK,
    &KEYBOARD,
    &MOUSE,
    &SCREEN,
    &DESK,
    &COFFEE_MACHINE,
    &FRIDGE,
    &LAMP_TABLE,
    &LAMP_FLOOR,
    &LAMP_ROUND_FLOOR,
    &LAPTOP,
    &LOUNGE_CHAIR,
    &LOUNGE_CHAIR_RELAX,
    &LOUNGE_SOFA,
    &LOUNGE_SOFA_LONG,
    &LOUNGE_DESIGN_SOFA,
    &PLANTS_SMALL[0],
    &PLANTS_SMALL[1],
    &PLANTS_SMALL[2],
    &POTTED_PLANT,
    &RUG,
    &RUG_ROUND,
    &SIDE_TABLE,
    &COFFEE_TABLE,
    &COFFEE_TABLE_GLASS,
    &TABLE,
    &TABLE_GLASS,
    &TABLE_ROUND,
    &TV,
    &CABINET_TV,
    &STOOL_BAR,
    &BOX_CLOSED,
    &BENCH_CUSHION,
    &TRASHCAN,
    &TRASHCAN,
];

/// Surface looks. Theme-dependent entries (`Wall`, `Trim`, `Accent`, `Fabric*`)
/// resolve against the floor's palette; the rest are fixed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mat {
    Wall,
    Trim,
    Accent,
    Fabric0,
    Fabric1,
    Fabric2,
    Board,
    Scr0,
    Scr1,
    Scr2,
    Water,
    Runner,
    RunnerEdge,
    Cable,
    Rack,
    RackPanel,
    LedGreen,
    LedRed,
    LedBlue,
    LightPanel,
    Glass,
    Gold,
    Dark,
    Metal,
    Wood,
    Door,
    Beige,
    NeonPink,
    NeonCyan,
    NeonYellow,
    Candle,
    ScreenGlow,
    BeanA,
    BeanB,
    BeanC,
    Felt,
    Cardboard,
    Paper,
    Yoga,
    Red,
    Yellow,
    White,
    Marble,
    MarbleDark,
    Plant,
    Dead,
    Monolith,
}

/// Procedurally drawn textures (see `textures.rs`); several carry short text
/// rendered with a built-in 5x7 bitmap font.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tex {
    PosterBars,
    PosterBurn,
    PosterHang,
    PosterPie,
    AiFirst,
    Synergy,
    AiTransformation,
    Flowchart,
    DaysCounter,
    MandatoryFun,
    Gantt,
    Stickies,
    PromptTips,
    OutOfOrder,
    HrDoor,
    Family,
    PizzaBox,
    ChatbotFace,
    Suggestions,
    CloseDeals,
    Skyline,
    Documentation,
    Wellness,
    Cobweb,
    Standup,
    DemoScreen,
    WarRoom,
    PraiseModel,
    Vending,
    Kpi,
    Leaderboard,
    Roadmap,
    Brand,
    MoodBoards,
    Campaign,
    SocAlert,
    Phishing,
    StickyPass,
    Misc,
    TurnItOff,
    NextAgent,
}

#[derive(Clone, Debug)]
pub enum Piece {
    Box {
        center: Vec3,
        size: Vec3,
        mat: Mat,
        solid: bool,
    },
    Prop {
        model: &'static Model,
        pos: Vec3,
        yaw: f32,
        solid: bool,
        /// Multiplies `FURNITURE_SCALE`.
        scale: f32,
    },
    /// Textured quad facing +Z at yaw 0.
    Panel {
        center: Vec3,
        yaw: f32,
        size: Vec2,
        tex: Tex,
    },
    Light {
        pos: Vec3,
        color: Color,
        intensity: f32,
    },
    /// Invisible blocker (floating chair colliders and the like).
    Block(Aabb),
    /// Box turned by any yaw; visual only (colliders are separate `Block`s).
    Slab {
        center: Vec3,
        yaw: f32,
        size: Vec3,
        mat: Mat,
    },
    /// Vertical band along curved walls, `inset` metres in front of the wall face.
    Run {
        segs: Vec<Seg>,
        mat: Mat,
        y0: f32,
        y1: f32,
        inset: f32,
    },
    /// Wall-top strip, only seen from above (debug overview).
    Cap { segs: Vec<Seg> },
}

/// Direction a wall-mounted item faces (into the room).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    PosX,
    NegX,
    PosZ,
    NegZ,
}

impl Face {
    pub fn yaw(self) -> f32 {
        match self {
            Face::PosZ => 0.0,
            Face::PosX => FRAC_PI_2,
            Face::NegZ => PI,
            Face::NegX => -FRAC_PI_2,
        }
    }

    pub fn dir(self) -> Vec3 {
        Quat::from_rotation_y(self.yaw()) * Vec3::Z
    }
}

/// World AABB size of a box of `size` turned by `yaw` about Y.
pub fn rotated_footprint(size: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(
        size.x * c.abs() + size.z * s.abs(),
        size.y,
        size.x * s.abs() + size.z * c.abs(),
    )
}

pub struct Anchor {
    pub name: &'static str,
    pub pos: Vec3,
    /// Unit yaw: `Quat::from_rotation_y(yaw)` applied to a unit root makes it
    /// face `(-sin yaw, 0, -cos yaw)`, same as the player body.
    pub yaw: f32,
}

#[derive(Default)]
pub struct Plan {
    pub pieces: Vec<Piece>,
    pub anchors: Vec<Anchor>,
    pub fp: Footprint,
    pub walls: Vec<Seg>,
    /// Centreline of the intended walk from the spawn to the boss pod (tests and pack placement).
    pub route: Vec<Vec2>,
    frame: Vec<(Vec3, f32)>,
}

impl Plan {
    fn to_world(&self, mut p: Vec3) -> Vec3 {
        for (o, yaw) in self.frame.iter().rev() {
            p = *o + Quat::from_rotation_y(*yaw) * p;
        }
        p
    }

    fn yaw_sum(&self) -> f32 {
        self.frame.iter().map(|f| f.1).sum()
    }

    /// Local coordinates relative to `origin`, turned by `yaw` (quarter turns
    /// only, so boxes stay axis aligned).
    pub fn framed(&mut self, origin: Vec3, yaw: f32, f: impl FnOnce(&mut Plan)) {
        self.frame.push((origin, yaw));
        f(self);
        self.frame.pop();
    }

    pub fn boxed(&mut self, center: Vec3, size: Vec3, mat: Mat, solid: bool) {
        let yaw = self.yaw_sum();
        let center = self.to_world(center);
        let size = rotated_footprint(size, yaw);
        self.pieces.push(Piece::Box {
            center,
            size,
            mat,
            solid,
        });
    }

    pub fn prop(&mut self, model: &'static Model, pos: Vec3, yaw: f32, solid: bool) {
        self.prop_scaled(model, pos, yaw, solid, 1.0);
    }

    pub fn prop_scaled(
        &mut self,
        model: &'static Model,
        pos: Vec3,
        yaw: f32,
        solid: bool,
        scale: f32,
    ) {
        let (pos, yaw) = (self.to_world(pos), yaw + self.yaw_sum());
        self.pieces.push(Piece::Prop {
            model,
            pos,
            yaw,
            solid,
            scale,
        });
    }

    pub fn panel(&mut self, center: Vec3, yaw: f32, size: Vec2, tex: Tex) {
        let (center, yaw) = (self.to_world(center), yaw + self.yaw_sum());
        self.pieces.push(Piece::Panel {
            center,
            yaw,
            size,
            tex,
        });
    }

    pub fn light(&mut self, pos: Vec3, color: Color, intensity: f32) {
        let pos = self.to_world(pos);
        self.pieces.push(Piece::Light {
            pos,
            color,
            intensity,
        });
    }

    pub fn block(&mut self, center: Vec3, size: Vec3) {
        let yaw = self.yaw_sum();
        let center = self.to_world(center);
        self.pieces
            .push(Piece::Block(aabb(center, rotated_footprint(size, yaw))));
    }

    pub fn anchor(&mut self, name: &'static str, pos: Vec3, yaw: f32) {
        self.anchors.push(Anchor { name, pos, yaw });
    }

    pub fn slab(&mut self, center: Vec3, yaw: f32, size: Vec3, mat: Mat) {
        self.pieces.push(Piece::Slab {
            center,
            yaw,
            size,
            mat,
        });
    }

    /// Turns the footprint into wall strips plus the AABB collider chain. Call once the
    /// outline is final; the `*_near` helpers below snap decor to these walls.
    pub fn build_walls(&mut self) {
        self.walls = self.fp.walls();
        let mats: Vec<Mat> = self.walls.iter().fold(Vec::new(), |mut m, s| {
            if !m.contains(&s.mat) {
                m.push(s.mat);
            }
            m
        });
        for mat in mats {
            let segs: Vec<Seg> = self
                .walls
                .iter()
                .filter(|s| s.mat == mat)
                .copied()
                .collect();
            let mut run = |mat, y0, y1, inset| {
                self.pieces.push(Piece::Run {
                    segs: segs.clone(),
                    mat,
                    y0,
                    y1,
                    inset,
                });
            };
            run(mat, 0.0, H, 0.0);
            if mat == Mat::Glass {
                run(Mat::Trim, 0.0, 0.12, 0.02);
                run(Mat::Trim, H - 0.12, H, 0.02);
            } else {
                run(Mat::Trim, 0.0, 0.2, 0.02);
                run(Mat::Accent, 2.87, 2.93, 0.02);
                self.pieces.push(Piece::Cap { segs });
            }
        }
        let mut boxes = colliders(&self.walls, COLLIDER_T);
        boxes.extend(self.fp.thin_colliders());
        self.pieces.extend(boxes.into_iter().map(Piece::Block));
    }

    /// Nearest opaque wall point and its inward normal.
    pub fn wall_near(&self, x: f32, z: f32) -> Option<(Vec2, Vec2)> {
        let q = Vec2::new(x, z);
        self.walls
            .iter()
            .filter(|s| s.mat != Mat::Glass)
            .map(|s| {
                let d = s.b - s.a;
                let t = ((q - s.a).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
                let pt = s.a + d * t;
                (pt.distance(q), pt, s.n)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, pt, n)| (pt, n))
    }

    /// Panel hung on the nearest wall at height `y`, facing into the room.
    pub fn panel_near(&mut self, x: f32, z: f32, y: f32, size: Vec2, tex: Tex) {
        if let Some((pt, n)) = self.wall_near(x, z) {
            let c = pt + n * 0.03;
            self.panel(Vec3::new(c.x, y, c.y), n.x.atan2(n.y), size, tex);
        }
    }

    /// Framed poster on the nearest wall.
    pub fn poster_near(&mut self, x: f32, z: f32, y: f32, size: Vec2, tex: Tex) {
        if let Some((pt, n)) = self.wall_near(x, z) {
            let yaw = n.x.atan2(n.y);
            let f = pt + n * 0.025;
            self.slab(
                Vec3::new(f.x, y, f.y),
                yaw,
                Vec3::new(size.x + 0.1, size.y + 0.1, 0.05),
                Mat::Trim,
            );
            let c = pt + n * 0.055;
            self.panel(Vec3::new(c.x, y, c.y), yaw, size, tex);
        }
    }

    /// Skyline window with mullions on the nearest wall.
    pub fn window_near(&mut self, x: f32, z: f32, size: Vec2, tex: Tex, mullion: Mat) {
        let Some((pt, n)) = self.wall_near(x, z) else {
            return;
        };
        let yaw = n.x.atan2(n.y);
        let lat = Vec2::new(yaw.cos(), -yaw.sin());
        let at = |depth: f32, l: f32, y: f32| {
            let c = pt + n * depth + lat * l;
            Vec3::new(c.x, y, c.y)
        };
        self.panel(at(0.03, 0.0, 2.4), yaw, size, tex);
        for l in [-size.x / 2.0, 0.0, size.x / 2.0] {
            self.slab(
                at(0.06, l, 2.4),
                yaw,
                Vec3::new(0.12, size.y + 0.2, 0.1),
                mullion,
            );
        }
        for y in [2.4 + size.y / 2.0 + 0.05, 2.4 - size.y / 2.0 - 0.05] {
            self.slab(
                at(0.06, 0.0, y),
                yaw,
                Vec3::new(size.x + 0.1, 0.12, 0.12),
                mullion,
            );
        }
    }

    /// Elevator set into the nearest wall. Returns the arrival point 1.8 m in front and the
    /// unit yaw that faces into the room.
    pub fn elevator_near(&mut self, x: f32, z: f32, door: Mat) -> (Vec3, f32) {
        let (pt, n) = self.wall_near(x, z).unwrap_or((Vec2::new(x, z), Vec2::X));
        let yaw = n.x.atan2(n.y);
        // Local x along the wall, local z along the normal.
        let lat = Vec2::new(yaw.cos(), -yaw.sin());
        let at = |depth: f32, l: f32, y: f32| {
            let c = pt + n * depth + lat * l;
            Vec3::new(c.x, y, c.y)
        };
        self.slab(at(0.1, 0.0, 1.55), yaw, Vec3::new(2.8, 3.1, 0.2), Mat::Dark);
        for l in [-0.5, 0.5] {
            self.slab(at(0.24, l, 1.2), yaw, Vec3::new(0.98, 2.4, 0.06), door);
        }
        self.slab(
            at(0.22, 1.9, 1.2),
            yaw,
            Vec3::new(0.2, 0.3, 0.08),
            Mat::Dark,
        );
        self.slab(
            at(0.27, 1.9, 1.25),
            yaw,
            Vec3::new(0.08, 0.08, 0.04),
            Mat::LedGreen,
        );
        self.slab(
            at(0.24, 0.0, 2.75),
            yaw,
            Vec3::new(1.2, 0.1, 0.06),
            Mat::LedBlue,
        );
        let arrive = pt + n * 1.8;
        (Vec3::new(arrive.x, 0.0, arrive.y), (-n.x).atan2(-n.y))
    }

    /// Framed panel on a wall: `face` is the direction the picture looks.
    pub fn wall_panel(&mut self, pos: Vec3, face: Face, size: Vec2, tex: Tex) {
        self.panel(pos + face.dir() * 0.03, face.yaw(), size, tex);
    }

    /// Poster with a dark frame, hung at height `y`.
    pub fn poster(&mut self, x: f32, z: f32, y: f32, face: Face, size: Vec2, tex: Tex) {
        let pos = Vec3::new(x, y, z);
        let d = face.dir();
        let flat = if d.x.abs() > 0.5 { 0.05 } else { size.x + 0.1 };
        let flat_z = if d.x.abs() > 0.5 { size.x + 0.1 } else { 0.05 };
        self.boxed(pos, Vec3::new(flat, size.y + 0.1, flat_z), Mat::Trim, false);
        self.wall_panel(pos, face, size, tex);
    }

    /// Wall running along Z at `x` from `z0` to `z1`, minus the `gaps` (door openings).
    /// Door gaps get a lintel so they read as doorways.
    pub fn wall_x(&mut self, x: f32, z0: f32, z1: f32, gaps: &[(f32, f32)], mat: Mat, h: f32) {
        for (a, b) in segments(z0, z1, gaps) {
            self.boxed(
                Vec3::new(x, h / 2.0, (a + b) / 2.0),
                Vec3::new(WALL_T, h, b - a),
                mat,
                true,
            );
        }
        for (a, b) in gaps {
            self.lintel(
                Vec3::new(x, 0.0, (a + b) / 2.0),
                Vec3::new(WALL_T, 0.0, b - a),
                h,
                mat,
            );
        }
    }

    /// Wall running along X at `z` from `x0` to `x1`, minus the `gaps`.
    pub fn wall_z(&mut self, z: f32, x0: f32, x1: f32, gaps: &[(f32, f32)], mat: Mat, h: f32) {
        for (a, b) in segments(x0, x1, gaps) {
            self.boxed(
                Vec3::new((a + b) / 2.0, h / 2.0, z),
                Vec3::new(b - a, h, WALL_T),
                mat,
                true,
            );
        }
        for (a, b) in gaps {
            self.lintel(
                Vec3::new((a + b) / 2.0, 0.0, z),
                Vec3::new(b - a, 0.0, WALL_T),
                h,
                mat,
            );
        }
    }

    fn lintel(&mut self, base: Vec3, span: Vec3, h: f32, mat: Mat) {
        if h > DOOR_H {
            self.boxed(
                Vec3::new(base.x, (h + DOOR_H) / 2.0, base.z),
                Vec3::new(span.x, h - DOOR_H, span.z),
                mat,
                false,
            );
        }
    }

    /// Elevator set into a side wall. `wall_x` is the inner wall face, `face` the
    /// direction the doors look. Returns the arrival point 1.8 m in front.
    pub fn elevator(&mut self, wall_x: f32, z: f32, face: Face, door: Mat) -> Vec3 {
        let s = face.dir().x;
        let at = |depth: f32, dz: f32, y: f32| Vec3::new(wall_x + s * depth, y, z + dz);
        // Frame, two door panels, a call panel and a floor indicator strip.
        self.boxed(
            at(0.1, 0.0, 1.55),
            Vec3::new(0.2, 3.1, 2.8),
            Mat::Dark,
            false,
        );
        for dz in [-0.5, 0.5] {
            self.boxed(at(0.24, dz, 1.2), Vec3::new(0.06, 2.4, 0.98), door, false);
        }
        self.boxed(
            at(0.22, 1.9, 1.2),
            Vec3::new(0.08, 0.3, 0.2),
            Mat::Dark,
            false,
        );
        self.boxed(
            at(0.27, 1.9, 1.25),
            Vec3::new(0.04, 0.08, 0.08),
            Mat::LedGreen,
            false,
        );
        self.boxed(
            at(0.24, 0.0, 2.75),
            Vec3::new(0.06, 0.1, 1.2),
            Mat::LedBlue,
            false,
        );
        at(1.8, 0.0, 0.0)
    }
}

pub const WALL_T: f32 = COLLIDER_T;
pub const DOOR_H: f32 = 2.5;
pub const H: f32 = WALL_HEIGHT;

/// `[lo, hi]` minus the sorted gap intervals.
fn segments(lo: f32, hi: f32, gaps: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut cur = lo;
    for &(a, b) in gaps {
        if a > cur {
            out.push((cur, a));
        }
        cur = b;
    }
    if hi > cur {
        out.push((cur, hi));
    }
    out
}
