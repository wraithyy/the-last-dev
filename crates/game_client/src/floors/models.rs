//! Kenney kit models used outdoors (CC0, see assets/CREDITS.md): city buildings, cars, trees,
//! rocks, boats, trains. Bounds measured from the glTF accessors like the furniture models.
//! `put` places one at a size in metres per model unit; every instance of a model shares its
//! mesh and material, so repeated trees, rocks and cars batch.

use super::kit::{FURNITURE_SCALE, Model, Plan};
use bevy::prelude::*;

pub static BUILDING_A: Model = Model::kit(
    "city-kit-commercial",
    "building-a",
    [-0.442, -0.470],
    [0.884, 1.293, 0.940],
);
pub static BUILDING_G: Model = Model::kit(
    "city-kit-commercial",
    "building-g",
    [-0.485, -0.461],
    [0.970, 1.693, 0.922],
);
pub static BUILDING_J: Model = Model::kit(
    "city-kit-commercial",
    "building-j",
    [-1.042, -0.670],
    [2.084, 1.693, 1.340],
);
pub static BUILDING_N: Model = Model::kit(
    "city-kit-commercial",
    "building-n",
    [-1.160, -0.910],
    [2.320, 2.480, 1.820],
);
pub static TOWER_B: Model = Model::kit(
    "city-kit-commercial",
    "building-skyscraper-b",
    [-0.680, -0.680],
    [1.360, 4.480, 1.360],
);
pub static TOWER_D: Model = Model::kit(
    "city-kit-commercial",
    "building-skyscraper-d",
    [-0.640, -0.694],
    [1.280, 5.470, 1.388],
);
pub static PARASOL_A: Model = Model::kit(
    "city-kit-commercial",
    "detail-parasol-a",
    [-0.173, -0.200],
    [0.346, 0.450, 0.400],
);
pub static HOUSE_A: Model = Model::kit(
    "city-kit-suburban",
    "building-type-a",
    [-0.650, -0.514],
    [1.300, 0.834, 1.028],
);
pub static HOUSE_N: Model = Model::kit(
    "city-kit-suburban",
    "building-type-n",
    [-0.892, -0.689],
    [1.784, 1.138, 1.378],
);
pub static HOUSE_T: Model = Model::kit(
    "city-kit-suburban",
    "building-type-t",
    [-0.664, -0.703],
    [1.314, 1.156, 1.406],
);
pub static FENCE_1X4: Model = Model::kit(
    "city-kit-suburban",
    "fence-1x4",
    [-0.838, -0.200],
    [1.675, 0.270, 0.438],
);
pub static PLANTER: Model = Model::kit(
    "city-kit-suburban",
    "planter",
    [-0.200, -0.151],
    [0.400, 0.177, 0.300],
);
pub static WAREHOUSE_L: Model = Model::kit(
    "city-kit-industrial",
    "building-l",
    [-1.464, -0.711],
    [2.084, 1.925, 1.870],
);
pub static WAREHOUSE_Q: Model = Model::kit(
    "city-kit-industrial",
    "building-q",
    [-1.298, -0.874],
    [2.140, 0.880, 1.770],
);
pub static CHIMNEY_LARGE: Model = Model::kit(
    "city-kit-industrial",
    "chimney-large",
    [-0.540, -0.540],
    [1.080, 1.700, 1.080],
);
pub static WATER_TOWER: Model = Model::kit(
    "city-kit-industrial",
    "water-tower",
    [-0.426, -0.416],
    [0.852, 2.142, 0.832],
);
pub static TANK_LARGE: Model = Model::kit(
    "city-kit-industrial",
    "detail-tank-large",
    [-0.754, -0.894],
    [1.508, 0.962, 1.648],
);
pub static SHIPPING_CONTAINER_A: Model = Model::kit(
    "city-kit-industrial",
    "shipping-container-a",
    [-0.186, -0.411],
    [0.373, 0.348, 0.823],
);
pub static SHIPPING_CONTAINER_B: Model = Model::kit(
    "city-kit-industrial",
    "shipping-container-b",
    [-0.186, -0.411],
    [0.373, 0.348, 0.823],
);
pub static SOLAR_PANELS: Model = Model::kit(
    "city-kit-industrial",
    "solar-panel-landscape-group",
    [-0.757, -0.448],
    [1.513, 0.262, 0.895],
);
pub static SEDAN: Model = Model::kit("car-kit", "sedan", [-0.750, -1.300], [1.500, 1.300, 2.550]);
pub static SUV: Model = Model::kit("car-kit", "suv", [-0.750, -1.350], [1.500, 1.300, 2.700]);
pub static SUV_LUXURY: Model = Model::kit(
    "car-kit",
    "suv-luxury",
    [-0.750, -1.400],
    [1.500, 1.300, 2.850],
);
pub static HATCHBACK_SPORTS: Model = Model::kit(
    "car-kit",
    "hatchback-sports",
    [-0.650, -1.450],
    [1.300, 1.100, 2.850],
);
pub static VAN: Model = Model::kit("car-kit", "van", [-0.750, -1.400], [1.500, 1.350, 2.750]);
pub static TAXI: Model = Model::kit("car-kit", "taxi", [-0.750, -1.400], [1.500, 1.500, 2.750]);
pub static DELIVERY: Model = Model::kit(
    "car-kit",
    "delivery",
    [-0.750, -1.650],
    [1.500, 1.650, 3.250],
);
pub static TRACTOR_SHOVEL: Model = Model::kit(
    "car-kit",
    "tractor-shovel",
    [-0.830, -1.235],
    [1.661, 1.512, 2.470],
);
pub static CONE: Model = Model::kit("car-kit", "cone", [-0.238, -0.238], [0.476, 0.595, 0.476]);
pub static TREE_OAK: Model = Model::kit(
    "nature-kit",
    "tree_oak",
    [-0.320, -0.370],
    [0.641, 1.226, 0.740],
);
pub static TREE_DEFAULT: Model = Model::kit(
    "nature-kit",
    "tree_default",
    [-0.378, -0.327],
    [0.755, 1.708, 0.654],
);
pub static TREE_DETAILED: Model = Model::kit(
    "nature-kit",
    "tree_detailed",
    [-0.432, -0.372],
    [0.847, 1.332, 0.762],
);
pub static TREE_FAT: Model = Model::kit(
    "nature-kit",
    "tree_fat",
    [-0.378, -0.327],
    [0.755, 1.150, 0.654],
);
pub static TREE_PINE_ROUND: Model = Model::kit(
    "nature-kit",
    "tree_pineRoundA",
    [-0.309, -0.356],
    [0.617, 1.366, 0.712],
);
pub static TREE_PINE_TALL: Model = Model::kit(
    "nature-kit",
    "tree_pineTallA",
    [-0.179, -0.194],
    [0.386, 1.530, 0.392],
);
pub static ROCK_FLAT: Model = Model::kit(
    "nature-kit",
    "rock_largeA",
    [-0.392, -0.508],
    [0.785, 0.260, 1.015],
);
pub static ROCK_BIG: Model = Model::kit(
    "nature-kit",
    "rock_largeD",
    [-0.534, -0.515],
    [1.069, 0.568, 1.030],
);
pub static ROCK_TALL: Model = Model::kit(
    "nature-kit",
    "rock_tallB",
    [-0.382, -0.385],
    [0.765, 0.884, 0.770],
);
pub static ROCK_SMALL: Model = Model::kit(
    "nature-kit",
    "rock_smallC",
    [-0.180, -0.180],
    [0.361, 0.123, 0.361],
);
pub static SHORE_STONE: Model = Model::kit(
    "nature-kit",
    "stone_largeC",
    [-0.532, -0.508],
    [1.064, 0.321, 1.016],
);
pub static BUSH: Model = Model::kit(
    "nature-kit",
    "plant_bush",
    [-0.198, -0.198],
    [0.396, 0.244, 0.396],
);
pub static BUSH_LARGE: Model = Model::kit(
    "nature-kit",
    "plant_bushLarge",
    [-0.187, -0.168],
    [0.374, 0.243, 0.336],
);
pub static GRASS_LARGE: Model = Model::kit(
    "nature-kit",
    "grass_large",
    [-0.199, -0.212],
    [0.409, 0.254, 0.408],
);
pub static LILY_LARGE: Model = Model::kit(
    "nature-kit",
    "lily_large",
    [-0.136, -0.157],
    [0.272, 0.098, 0.314],
);
pub static FLOWER_YELLOW: Model = Model::kit(
    "nature-kit",
    "flower_yellowA",
    [-0.105, -0.091],
    [0.159, 0.193, 0.181],
);
pub static FLOWER_RED: Model = Model::kit(
    "nature-kit",
    "flower_redA",
    [-0.105, -0.091],
    [0.159, 0.292, 0.181],
);
pub static REEDS: Model = Model::kit(
    "nature-kit",
    "crops_bambooStageB",
    [-0.177, -0.161],
    [0.340, 0.890, 0.308],
);
pub static PATH_WOOD: Model = Model::kit(
    "nature-kit",
    "path_wood",
    [-0.500, -0.257],
    [1.000, 0.050, 0.514],
);
pub static LOG_LARGE: Model = Model::kit(
    "nature-kit",
    "log_large",
    [-0.500, -0.253],
    [1.000, 0.417, 0.549],
);
pub static CANOE: Model = Model::kit(
    "nature-kit",
    "canoe",
    [-0.150, -0.575],
    [0.300, 0.182, 1.150],
);
pub static BOAT_ROW_SMALL: Model = Model::kit(
    "watercraft-pack",
    "boat-row-small",
    [-1.374, -1.186],
    [2.749, 0.848, 2.372],
);
pub static BOAT_SAIL_A: Model = Model::kit(
    "watercraft-pack",
    "boat-sail-a",
    [-0.892, -1.785],
    [1.785, 4.739, 3.770],
);
pub static BUOY: Model = Model::kit(
    "watercraft-pack",
    "buoy",
    [-0.464, -0.464],
    [0.928, 1.844, 0.928],
);
pub static TRAIN_HEAD: Model = Model::kit(
    "train-kit",
    "train-electric-city-a",
    [-0.550, -1.200],
    [1.100, 1.683, 2.400],
);
pub static TRAIN_CAR: Model = Model::kit(
    "train-kit",
    "train-electric-city-c",
    [-0.550, -1.350],
    [1.100, 1.683, 2.700],
);
pub static BURGER: Model = Model::kit(
    "food-kit",
    "burger",
    [-0.202, -0.202],
    [0.404, 0.290, 0.404],
);
pub static SODA_CAN: Model = Model::kit(
    "food-kit",
    "soda-can",
    [-0.112, -0.112],
    [0.225, 0.351, 0.225],
);
pub static CUP_COFFEE: Model = Model::kit(
    "food-kit",
    "cup-coffee",
    [-0.108, -0.164],
    [0.216, 0.140, 0.287],
);
pub static BOX_LARGE: Model = Model::kit(
    "factory-kit",
    "box-large",
    [-0.550, -0.500],
    [1.100, 0.550, 1.000],
);
pub static ROBOT_ARM_A: Model = Model::kit(
    "factory-kit",
    "robot-arm-a",
    [-0.500, -0.500],
    [1.000, 3.200, 1.000],
);

#[cfg(test)]
pub static ALL: [&Model; 60] = [
    &BUILDING_A,
    &BUILDING_G,
    &BUILDING_J,
    &BUILDING_N,
    &TOWER_B,
    &TOWER_D,
    &PARASOL_A,
    &HOUSE_A,
    &HOUSE_N,
    &HOUSE_T,
    &FENCE_1X4,
    &PLANTER,
    &WAREHOUSE_L,
    &WAREHOUSE_Q,
    &CHIMNEY_LARGE,
    &WATER_TOWER,
    &TANK_LARGE,
    &SHIPPING_CONTAINER_A,
    &SHIPPING_CONTAINER_B,
    &SOLAR_PANELS,
    &SEDAN,
    &SUV,
    &SUV_LUXURY,
    &HATCHBACK_SPORTS,
    &VAN,
    &TAXI,
    &DELIVERY,
    &TRACTOR_SHOVEL,
    &CONE,
    &TREE_OAK,
    &TREE_DEFAULT,
    &TREE_DETAILED,
    &TREE_FAT,
    &TREE_PINE_ROUND,
    &TREE_PINE_TALL,
    &ROCK_FLAT,
    &ROCK_BIG,
    &ROCK_TALL,
    &ROCK_SMALL,
    &SHORE_STONE,
    &BUSH,
    &BUSH_LARGE,
    &GRASS_LARGE,
    &LILY_LARGE,
    &FLOWER_YELLOW,
    &FLOWER_RED,
    &REEDS,
    &PATH_WOOD,
    &LOG_LARGE,
    &CANOE,
    &BOAT_ROW_SMALL,
    &BOAT_SAIL_A,
    &BUOY,
    &TRAIN_HEAD,
    &TRAIN_CAR,
    &BURGER,
    &SODA_CAN,
    &CUP_COFFEE,
    &BOX_LARGE,
    &ROBOT_ARM_A,
];

/// One model with its footprint centred on `pos`, front (+Z) turned by `yaw`, `unit` metres
/// per model unit. Solid models block with their footprint box.
pub fn put(p: &mut Plan, m: &'static Model, pos: Vec3, yaw: f32, unit: f32, solid: bool) {
    p.prop_scaled(m, pos, yaw, solid, unit / FURNITURE_SCALE);
}

/// Cheap, stable variety from a position (no rng in layouts: they must be deterministic).
pub fn pick(x: f32, z: f32, n: usize) -> usize {
    let h = (x * 12.9898 + z * 78.233).sin() * 43_758.547;
    ((h - h.floor()) * n as f32) as usize % n.max(1)
}

/// A tree about `size` (1.5..2) times the old primitive scale: a broadleaf or a pine, picked
/// by position, with a trunk-sized collider.
pub fn tree(p: &mut Plan, x: f32, z: f32, pine: bool, size: f32) {
    let (m, unit): (&'static Model, f32) = if pine {
        [(&TREE_PINE_ROUND, 4.6), (&TREE_PINE_TALL, 4.4)][pick(x, z, 2)]
    } else {
        [
            (&TREE_OAK, 5.2),
            (&TREE_DEFAULT, 4.2),
            (&TREE_DETAILED, 5.0),
            (&TREE_FAT, 5.2),
        ][pick(x, z, 4)]
    };
    let yaw = pick(z, x, 8) as f32 * 0.8;
    put(p, m, Vec3::new(x, 0.0, z), yaw, unit * size / 1.7, false);
    p.block(Vec3::new(x, 1.5, z), Vec3::new(0.7, 3.0, 0.7));
}

/// A parked car, its paint picked from the old colour: long along x or z, nose either way.
pub fn car(p: &mut Plan, x: f32, z: f32, along_x: bool, paint: usize) {
    let m: &'static Model = [&SEDAN, &HATCHBACK_SPORTS, &SUV, &VAN, &TAXI, &SUV_LUXURY][paint % 6];
    let base = if along_x {
        std::f32::consts::FRAC_PI_2
    } else {
        0.0
    };
    let yaw = base + std::f32::consts::PI * pick(x, z, 2) as f32;
    put(p, m, Vec3::new(x, 0.0, z), yaw, 1.55, false);
    let size = if along_x {
        Vec3::new(4.4, 1.5, 1.8)
    } else {
        Vec3::new(1.8, 1.5, 4.4)
    };
    p.block(Vec3::new(x, 0.75, z), size);
}
