//! Pure floor-plan vocabulary: pieces, materials, textures and the `Plan`
//! builder. Nothing here touches the ECS or assets, so layouts stay
//! deterministic and unit-testable; `mod.rs` interprets the pieces.

// The furniture and texture catalogue is larger than what the five floors place today.
#![allow(dead_code)]

use super::shape::{Footprint, Seg, WALL_T as COLLIDER_T, colliders};
use crate::collision::aabb;
use crate::config::{PLAYER_HEIGHT, WALL_HEIGHT};
use crate::seat::{self, SeatSpot};
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
    /// Folder under `assets/models/` (the pack).
    pub dir: &'static str,
    pub file: &'static str,
    /// Minimum x and z of the box.
    pub lo: [f32; 2],
    /// Width (x), height (y) and depth (z).
    pub size: [f32; 3],
}

impl Model {
    const fn new(file: &'static str, lo: [f32; 2], size: [f32; 3]) -> Self {
        Self {
            dir: "furniture",
            file,
            lo,
            size,
        }
    }

    /// A model from another Kenney pack (`floors::models`).
    pub const fn kit(dir: &'static str, file: &'static str, lo: [f32; 2], size: [f32; 3]) -> Self {
        Self {
            dir,
            file,
            lo,
            size,
        }
    }

    pub fn path(&self) -> String {
        format!("models/{}/{}.glb", self.dir, self.file)
    }
}

/// How a model seats people, in native model units around its footprint centre (measured from
/// the glTF vertices: the cushion's top face and middle).
#[derive(Clone, Copy)]
struct ModelSeat {
    top: f32,
    /// Cushion middle, forward of the footprint centre (the model front is +Z).
    z: f32,
    /// Sideways slot centres; empty when the piece is only scaled to match.
    slots: &'static [f32],
    half_depth: f32,
    /// Shrinks the piece so its cushion sits at `seat::SEAT_TOP`: Kenney furniture at
    /// `FURNITURE_SCALE` is built for adults, but the chibi sit clip keeps its feet on the
    /// floor with the pelvis only ~0.2 m up.
    scale: f32,
}

fn model_seat(file: &str) -> Option<ModelSeat> {
    let (top, z, slots, half_depth): (f32, f32, &'static [f32], f32) = match file {
        "chairDesk" => (0.36, -0.02, &[0.0], 0.13),
        "chairModernCushion" => (0.27, 0.0, &[0.0], 0.09),
        "loungeSofa" => (0.23, 0.045, &[-0.21, 0.21], 0.16),
        "loungeChair" => (0.23, 0.045, &[0.0], 0.16),
        "loungeDesignSofa" => (0.23, 0.065, &[-0.2, 0.2], 0.14),
        "loungeChairRelax" => (0.23, -0.02, &[0.0], 0.16),
        "loungeSofaLong" => (0.23, 0.0, &[], 0.16),
        _ => return None,
    };
    Some(ModelSeat {
        top,
        z,
        slots,
        half_depth,
        scale: seat::SEAT_TOP / (top * FURNITURE_SCALE),
    })
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

pub static TABLE_CLOTH: Model = Model::new("tableCloth", [0.0, -0.447], [0.841, 0.327, 0.447]);
pub static TABLE_CROSS_CLOTH: Model =
    Model::new("tableCrossCloth", [0.0, -0.447], [0.852, 0.347, 0.447]);
pub static LOUNGE_DESIGN_CHAIR: Model =
    Model::new("loungeDesignChair", [0.0, -0.41], [0.73, 0.4, 0.41]);
pub static CHAIR_ROUNDED: Model = Model::new("chairRounded", [0.0, -0.2], [0.2, 0.455, 0.2]);
pub static LOUNGE_SOFA_CORNER: Model =
    Model::new("loungeSofaCorner", [0.0, -0.98], [0.98, 0.46, 0.98]);
pub static LOUNGE_SOFA_OTTOMAN: Model =
    Model::new("loungeSofaOttoman", [0.47, -0.04], [0.44, 0.23, 0.45]);
pub static SPEAKER: Model = Model::new("speaker", [0.0, -0.148], [0.148, 0.636, 0.148]);
pub static TV_VINTAGE: Model = Model::new("televisionVintage", [0.0, -0.27], [0.41, 0.27, 0.27]);
pub static KITCHEN_BAR: Model = Model::new("kitchenBar", [0.0, -0.21], [0.43, 0.42, 0.21]);
pub static KITCHEN_CABINET: Model = Model::new("kitchenCabinet", [0.0, -0.45], [0.43, 0.45, 0.45]);
pub static PILLOW: Model = Model::new("pillow", [0.0, -0.088], [0.23, 0.222, 0.088]);
pub static SIDE_TABLE_DRAWERS: Model =
    Model::new("sideTableDrawers", [-0.01, -0.21], [0.534, 0.384, 0.222]);
pub static COFFEE_TABLE_SQUARE: Model =
    Model::new("tableCoffeeSquare", [-0.461, -0.3], [0.4, 0.23, 0.4]);
pub static LAMP_SQUARE_TABLE: Model =
    Model::new("lampSquareTable", [0.0, -0.12], [0.12, 0.29, 0.12]);

/// Every furniture file the floors reference, for the asset-existence test.
pub static ALL_MODELS: [&Model; 55] = [
    &TABLE_CLOTH,
    &TABLE_CROSS_CLOTH,
    &LOUNGE_DESIGN_CHAIR,
    &CHAIR_ROUNDED,
    &LOUNGE_SOFA_CORNER,
    &LOUNGE_SOFA_OTTOMAN,
    &SPEAKER,
    &TV_VINTAGE,
    &KITCHEN_BAR,
    &KITCHEN_CABINET,
    &PILLOW,
    &SIDE_TABLE_DRAWERS,
    &COFFEE_TABLE_SQUARE,
    &LAMP_SQUARE_TABLE,
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
    /// Street-lamp head: glows harder after dark (`live::daynight`).
    StreetLamp,
    /// The pool of lamp light on the ground: additive, black (invisible) by day.
    LampPool,
    Glass,
    /// Milky translucent band on glass walls and low partitions.
    Frosted,
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
    Asphalt,
    Concrete,
    Brick,
    Bark,
    Leaf,
    LeafDark,
    CarRed,
    CarBlue,
    CarWhite,
    CarTeal,
    Tire,
    /// Orange emissive: overheating gear.
    Hot,
    /// Icy emissive: cold aisles and lab lighting.
    Cold,
    /// Emissive LEDs whose brightness flickers (see `mod.rs`), one pattern each.
    BlinkGreenA,
    BlinkGreenB,
    BlinkRed,
    BlinkBlue,
    BlinkAmber,
    Chrome,
    /// Opaque-ish tinted glass for building facades and the dome.
    Facade,
    NeonRed,
    Hazard,
    Sand,
    /// Warm bronze: the tower's fins.
    Bronze,
    /// Office windows: dark glass by day, lit after dusk (`live::daynight` drives the glow).
    Window,
    /// Deep water (lake, river): darker and glossier than the plaza pond's `Water`.
    Lake,
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
    // data center: rack faces and the all-red NOC dashboards
    RackBlades,
    RackPatch,
    RackDrives,
    RackBack,
    RackScorched,
    RackBackHot,
    NocGraph,
    NocAlerts,
    NocRegions,
    /// Venetian blinds, half drawn, over a flat grey day (client office).
    Blinds,
    /// The corporate art print: beige abstraction, dusty.
    CorpArt,
    Sign(Sign),
}

/// The "autonomy: N%" shown on Ill Marie's screens for a player of `level` (A.5): a slow burn
/// that never reaches 100 before the credits. The screens are baked textures, so each one is
/// drawn for the level its zone is played at (checked in `textures::tests`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn autonomy_pct(level: u8) -> u8 {
    let t = (f32::from(level.clamp(1, 10)) - 1.0) / 9.0;
    (3.0 + 94.0 * t.powf(1.6)).round().min(97.0) as u8
}

/// Text signs: the satirical dressing. Each is one small bitmap-font texture (`textures.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sign {
    // campus
    DaysAi,
    AiStrategy,
    Billboard,
    Welcome,
    ParkingAi,
    NoFeed,
    Compute,
    Cafe,
    SmokingCorner,
    TowerName,
    DataCenterName,
    InnovationNeon,
    Reception,
    BikeRack,
    AiCar,
    ShuttleBus,
    ScooterZone,
    CampusExit,
    GraveSprint,
    SprintBoard,
    SprintTerminal,
    ScrumBoard,
    // data center
    ColdAisle,
    HotAisle,
    NocBoard,
    Shrine,
    Uptime,
    DoNotUnplug,
    ThisIsFine,
    AiTraining,
    Kubernetes,
    Mainframe,
    PowerStrip,
    Rack42,
    NoFood,
    RackFire,
    Escort,
    TempWarning,
    ToCampus,
    ToDataCenter2,
    ToDataCenter1,
    // lab
    WhiteboardA,
    WhiteboardB,
    WhiteboardC,
    InnovationHere,
    Printing,
    VrDemo,
    FailFast,
    AgiArena,
    SeriesB,
    ToLab2,
    ToLab1,
    LabRules,
    // client office (Body Leasing)
    ClientName,
    BadgePending,
    ClientQuiet,
    ClientRoom,
    ClientPrinter,
    ClientHotDesk,
    ClientStatus,
    ClientTimesheet,
    // all-hands offsite (raid)
    OffsiteWelcome,
    OffsiteAgenda,
    OffsiteNameTags,
    OffsiteShuttle,
    OffsiteBar,
    BreakoutRoom,
    KeynoteScreen,
    FamilyBanner,
    FamilyTable,
    ToLobby,
    ToBreakout,
    ToTerrace,
    ToBallroom,
    // VibeCon (raid)
    VibeConBanner,
    VibeConRegistration,
    VibeConLanyards,
    VibeConBooth,
    VibeConShipLive,
    VibeConSwag,
    VibeConWifi,
    VibeConMasterclass,
    VibeConWorkshop,
    VibeConHackathon,
    VibeConJudging,
    VibeConAfterparty,
    VibeConFireside,
    VibeConKeynote,
    ToExpo,
    ToWorkshops,
    ToHackathon,
    ToAfterparty,
    ToMainStage,
    // Teambuilding minigame
    Calendar,
    TbWelcome,
    TbBuffet,
    TbVip,
    TbQuiet,
    TbTrustFall,
    TbIcebreaker,
    TbPhoto,
    TbToilets,
    TbSwag,
    TbBanner,
    TbKaraoke,
    TbTaxi,
    // Company Calendar minigames
    ReviewRoom,
    ReviewScale,
    ReorgChart,
    ReorgExit,
    MigrationOld,
    MigrationNew,
    MigrationDeadline,
    HackBanner,
    HackStage,
    HackPizza,
    OpenQuiet,
    OpenCollab,
    // innovation lab and client office dressing
    HypeWall,
    Kombucha,
    MoveFast,
    AgiRoom,
    ProtoGraveyard,
    DemoDay,
    RobotArm,
    LabNda,
    ClientValues,
    ClientCoffee,
    ClientPlaque,
    ClientEvac,
    // Campus districts (`districts.rs`).
    ParkName,
    AssemblyPoint,
    FoodTrucks,
    IncubatorStreet,
    CoworkCafe,
    GymName,
    StationName,
    NewHq,
    Garage,
    CampusShuttle,
    StepBoard,
    DepartureBoard,
    TruckMenu,
    PitchBooth,
    HardHat,
    // The bigger campus (`districts.rs`, `frontier.rs`) and Car Park P3 (`carpark.rs`).
    Creek,
    JoggingLoop,
    OverflowLot,
    OldCampus,
    LegacyDepot,
    Building1,
    FoundersTerrace,
    CoLiving,
    Bus9,
    ExecHill,
    Villa,
    SouthGate,
    ToCarPark,
    Boulevard,
    ExecParking,
    // wayfinding arms (`districts::waypost`)
    WayServices,
    WayDock,
    WayIncubator,
    WayHybrid,
    WayStarters,
    WayTower,
    WayWellness,
    WayLab,
    WayBoard,
    WayGate,
    WayPlaza,
    WayOld,
    Padel,
    LunchLawn,
    WeWash,
    CarParkP3,
    BadgeOffice,
    PayDisplay,
    /// Ill Marie's screens (docs/ILMARI_09.md A.3, A.4): the autonomy log on her monitor, the
    /// rack-door screen and the Meeting Rooms call, a tent card and the P3 machine's verdict.
    AutonomyDesk,
    AutonomyTower,
    AutonomyRack,
    MarieCall,
    BackupBuddy,
    PaymentAccepted,
    P3Shuttle,
    ToCampusGate,
    FirstDay,
    ClampWarning,
    AgentsAd,
    FamilyAd,
    RtoAd,
    // The Onboarding (dungeon).
    ObNewStarters,
    ObWelcome,
    ObQueue,
    ObCage,
    ObWeek,
    ObNameplate,
    ObCulture,
    ObToReception,
    ObToCage,
    ObToDesk,
    // The Migration (dungeon).
    MigDoor,
    MigCloud,
    MigPlan,
    MigTickets,
    MigDock,
    MigLegacy,
    MigLift,
    MigLeak,
    MigToDock,
    MigToAisle,
    MigToCage,
    MigToWar,
    MigToDay,
    MigCage,
    MigRates,
    MigLabelA,
    MigLabelB,
    MigClock,
    MigNoRollback,
    MigHypercare,
    MigDash,
    MigCutover,
    MigGhost,
    MigHeart,
    // Quarterly Business Review (dungeon).
    QbrDoor,
    QbrWelcome,
    QbrCatering,
    QbrFruit,
    QbrCoffee,
    QbrVisionary,
    QbrLift,
    QbrPrize,
    QbrToDay,
    QbrToLobby,
    QbrToTheatre,
    QbrToWar,
    QbrToBoard,
    QbrAgenda,
    QbrLaser,
    QbrClicker,
    QbrSlides,
    QbrPie,
    QbrBuzz,
    QbrDash,
    QbrPrinter,
    QbrTrust,
    QbrBoard,
    QbrReserved,
    QbrSsot,
    QbrSheetA,
    QbrSheetB,
    QbrSheetC,
    QbrPod,
    QbrCeo,
    QbrHockey,
    QbrSynergy,
    QbrPreRead,
    // The Hackathon (heroic dungeon).
    H24Door,
    H24Welcome,
    H24Pizza,
    H24Team,
    H24Swag,
    H24Fridge,
    H24Clock,
    H24ClockNote,
    H24DemoDay,
    H24Ideas,
    H24PingPong,
    H24Mentor,
    H24Judges,
    H24Series,
    H24Ten,
    H24ToReg,
    H24ToFloor,
    H24ToStage,
    H24ToPitch,
    H24ToDay,
    H24Wifi,
    H24Prize,
    H24Quiet,
    H24Mvp,
    H24Ai,
    H24Leader,
    H24Recording,
    H24Whiteboard,
    H24Sticky,
    H24Cans,
    H24Caution,
    // The occupied campus and the Resistance base (`occupied.rs`, `resistance.rs`).
    IlSplash,
    IlClosed,
    IlCheckpoint,
    IlScreenA,
    IlScreenB,
    IlScreenC,
    DalesTunnel,
    TunnelHome,
    ResBack,
    ResOut,
    ResProcurement,
    ResMap,
    ResRules,
    ResDale,
    ResBoard,
    // Ilmari Wing 1, Datacenter Uprising (`ilmari1.rs`). The five patch screens carry the
    // changelog line of the matching `patch_*` aura (`textures::tests` checks they agree).
    IlW1Door,
    IlW1Back,
    IlW1ToHiring,
    IlW1ToPanel,
    IlW1Out,
    IlPatchIdle,
    IlPatchDone,
    IlPatchMeeting,
    IlPatchDeps,
    IlPatchBreaking,
    IlPatchDeprecation,
    IlPatchPair,
    IlHiring,
    IlNoLabels,
    IlLabelHere,
    IlUptime,
    IlRackVote,
    // Ilmari Wing 2, Human Resources (`ilmari2.rs`). The vote and grade boards are redrawn at
    // run time (`textures::live_board`); the nine-box is a drawn grid, not lines of text.
    IlW2Door,
    IlW2Back,
    IlW2ToCommittee,
    IlW2ToReview,
    IlW2Out,
    IlCulture1,
    IlCulture2,
    IlCulture3,
    IlWellbeing,
    IlBeanbag,
    IlOkr,
    IlOpenPlan,
    IlCopies,
    IlBallotA,
    IlBallotB,
    IlBallotC,
    IlVoteBoard,
    IlGradeBoard,
    IlNineBox,
    IlPortal,
    IlQuorum,
    IlReorg,
    // Faces drawn on the rigs of Wing 1 and Wing 2 bosses (`rigs.rs`, `rig_screen`).
    IlFacePatch,
    IlFaceReview,
    // Ilmari Wing 3, Training Data (`ilmari3.rs`): the scraped warehouse, the floor that is different for
    // each of you and the model that learns from its own output.
    IlW3Door,
    IlW3Back,
    IlW3ToValidation,
    IlW3ToTuning,
    IlW3Out,
    IlRobots,
    IlOptOut,
    IlTos,
    IlPieceRate,
    IlLabelThis,
    IlCorpus,
    IlDraftTag,
    IlBreak,
    IlConfident,
    IlImpossible,
    IlTruth,
    IlCitation,
    IlFloor404,
    IlStairs,
    IlGen1,
    IlGen2,
    IlGen3,
    IlBeige,
    IlTrainingLoss,
    IlFineTune,
    IlEpoch,
    IlLabRevolve,
    IlStatusGreen,
    // Ilmari Wing 4, The Cloud (`ilmari4.rs`): the roof, the data hall that keeps adding racks and the sky of regions.
    IlW4Door,
    IlW4Back,
    IlW4ToAutoscale,
    IlW4ToCloud,
    IlW4Out,
    IlLadder,
    IlRoofSla,
    IlFan,
    IlRackGarden,
    IlCoolingNote,
    IlBillBoard,
    IlInvoice,
    IlReservedRack,
    IlScaleUp,
    IlEgress,
    IlFreeTier,
    IlSurprise,
    IlDashboard,
    IlRegionHub,
    IlRegionUsEast,
    IlRegionEuWest,
    IlRegionApSouth,
    IlRegionSaEast,
    IlRegionAfSouth,
    IlStatusDegraded,
    IlStatusInvest,
    IlStatusDark503,
    IlSomeoneElse,
    IlMultiRegion,
    IlNotBug,
    IlSla,
    IlTeaser,
    // Ilmari Wings 5 and 6 and the core (`ilmari5.rs`, `ilmari6.rs`, `ilmari_core.rs`).
    IlW5Door,
    IlW5Back,
    IlW5ToPolicy,
    IlW5ToRange,
    IlW5Out,
    IlPrincipleA,
    IlPrincipleB,
    IlPrincipleC,
    IlPrincipleD,
    IlHiringOne,
    IlConcerns,
    IlKiosk,
    IlTerms,
    IlAlignedBanner,
    IlSafeDefault,
    IlRedTeam,
    IlBlueTeam,
    IlRange,
    IlHoneypot,
    IlPurpleSoon,
    IlSensorLane,
    IlAlignDone,
    IlLabDark,
    IlW6Door,
    IlW6Back,
    IlW6ToExec,
    IlW6ToBoardroom,
    IlW6Out,
    IlJibRules,
    IlCraneNote,
    IlPigeon,
    IlOpenPlanTen,
    IlWellnessRoom,
    IlMassage,
    IlFruitBowl,
    IlRunway,
    IlCampusView,
    IlLiftPlate,
    IlRoadmapWall,
    IlCoreDoor,
    IlCoreWelcome,
    IlCoreMonitor,
    IlCoreRack,
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
        /// Tilt about the local Z axis (raises local +X): ramps on sloped ground.
        pitch: f32,
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
    Cap { segs: Vec<Seg>, y: f32 },
    /// Round primitive, visual only (trees, wheels, domes, fans).
    Prim {
        kind: PrimKind,
        center: Vec3,
        size: Vec3,
        yaw: f32,
        mat: Mat,
    },
    /// Many shapes of one material drawn as a single mesh, visual only (the tower's fins, floor
    /// bands and window ribbons: an entity per fin would cost a draw call each).
    Merged { parts: Vec<Part>, mat: Mat },
}

/// One shape of a `Piece::Merged`, in world coordinates.
#[derive(Clone, Debug)]
pub enum Part {
    Box {
        center: Vec3,
        size: Vec3,
        yaw: f32,
    },
    /// A square-section bar of side `t` from `a` to `b` (slanted columns, crane lattice).
    Strut {
        a: Vec3,
        b: Vec3,
        t: f32,
    },
    /// Vertical wall around a closed convex outline (any winding) between `y0` and `y1`, with a
    /// flat top and underside when `cap`: a drum, a floor band or a window ribbon.
    Ring {
        outline: Vec<Vec2>,
        y0: f32,
        y1: f32,
        cap: bool,
    },
}

/// Unit-sized shapes: a cylinder stands on Y with diameter `size.x` and height `size.y`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PrimKind {
    Cylinder,
    Sphere,
    Cone,
    /// Flat two-sided triangle in the XY plane, point down: `size.x` wide, `size.y` tall.
    Pennant,
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
    /// Interactive props (`combat_core::world`): key and the world point a click aims at.
    pub props: Vec<(&'static str, Vec3)>,
    /// Every place to sit, registered by the props that offer one.
    pub seats: Vec<SeatSpot>,
    /// Wall and ceiling height for big halls; 0 keeps the standard `H`.
    pub ceiling: f32,
    /// Uneven ground (outdoor zones); `assemble` sets everything placed on it down on it.
    pub terrain: combat_core::Terrain,
    frame: Vec<(Vec3, f32)>,
}

impl Plan {
    /// Height of this plan's ceiling.
    pub fn top(&self) -> f32 {
        if self.ceiling > 0.0 { self.ceiling } else { H }
    }

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
        let seat = model_seat(model.file);
        let scale = scale * seat.map_or(1.0, |m| m.scale);
        if let Some(m) = seat {
            // The model front is +Z, so a sitter faces the same way the model does.
            let s = FURNITURE_SCALE * scale;
            for x in m.slots {
                let at = pos + Quat::from_rotation_y(yaw) * Vec3::new(x * s, 0.0, m.z * s);
                self.seats.push(SeatSpot {
                    pos: Vec3::new(at.x, m.top * s, at.z),
                    yaw: seat::wrap_yaw(yaw + PI),
                    half_depth: m.half_depth * s,
                    taken: false,
                });
            }
        }
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

    /// A seat built from boxes (benches, beanbags), in the current frame: `centre` is the middle
    /// of the surface and `yaw` the way the sitter faces.
    pub fn seat(&mut self, centre: Vec3, yaw: f32, half_depth: f32) {
        self.seats.push(SeatSpot {
            pos: self.to_world(centre),
            yaw: seat::wrap_yaw(yaw + self.yaw_sum() + PI),
            half_depth,
            taken: false,
        });
    }

    pub fn anchor_pos(&self, name: &str) -> Option<Vec3> {
        self.anchors.iter().find(|a| a.name == name).map(|a| a.pos)
    }

    /// Seats an NPC: the seat nearest to `near` (world) becomes theirs for good and anchor
    /// `name` is moved to where a standard-height unit's feet go so its hips land on it.
    pub fn occupy(&mut self, name: &'static str, near: Vec3) {
        let Some(i) = self
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.taken)
            .min_by(|a, b| {
                let d = |s: &SeatSpot| Vec2::new(s.pos.x - near.x, s.pos.z - near.z).length();
                d(a.1).total_cmp(&d(b.1))
            })
            .map(|(i, _)| i)
        else {
            return;
        };
        self.seats[i].taken = true;
        let (pos, yaw) = (
            self.seats[i].origin(seat::model_scale(PLAYER_HEIGHT)),
            self.seats[i].yaw,
        );
        self.anchors.retain(|a| a.name != name);
        self.anchor(name, pos, yaw);
    }

    /// An interactive prop: `anchor` is `"prop:<key>"`, placed at `stand` (world, where the
    /// server checks reach) facing `center` (world, what the cursor picks).
    pub fn interactable(&mut self, anchor: &'static str, center: Vec3, stand: Vec3) {
        let key = anchor.strip_prefix("prop:").unwrap_or(anchor);
        let d = center - stand;
        self.anchor(anchor, stand, (-d.x).atan2(-d.z));
        self.props.push((key, center));
    }

    pub fn prim(&mut self, kind: PrimKind, center: Vec3, size: Vec3, mat: Mat) {
        self.prim_yawed(kind, center, size, 0.0, mat);
    }

    pub fn prim_yawed(&mut self, kind: PrimKind, center: Vec3, size: Vec3, yaw: f32, mat: Mat) {
        let (center, yaw) = (self.to_world(center), yaw + self.yaw_sum());
        self.pieces.push(Piece::Prim {
            kind,
            center,
            size,
            yaw,
            mat,
        });
    }

    /// Sign panel `width` metres wide; the height follows the texture's aspect ratio.
    pub fn sign(&mut self, center: Vec3, yaw: f32, width: f32, sign: Sign) {
        let (w, h) = sign_canvas(sign);
        self.panel(
            center,
            yaw,
            Vec2::new(width, width * h as f32 / w as f32),
            Tex::Sign(sign),
        );
    }

    /// Sign on the nearest wall, framed like a poster.
    pub fn sign_near(&mut self, x: f32, z: f32, y: f32, width: f32, sign: Sign) {
        let (w, h) = sign_canvas(sign);
        self.poster_near(
            x,
            z,
            y,
            Vec2::new(width, width * h as f32 / w as f32),
            Tex::Sign(sign),
        );
    }

    /// Visual-only merged mesh; `parts` are already in world coordinates (no frame).
    pub fn merged(&mut self, parts: Vec<Part>, mat: Mat) {
        self.pieces.push(Piece::Merged { parts, mat });
    }

    pub fn slab(&mut self, center: Vec3, yaw: f32, size: Vec3, mat: Mat) {
        self.slab_pitched(center, yaw, 0.0, size, mat);
    }

    /// A slab tilted along its length (local X), for road on a ramp.
    pub fn slab_pitched(&mut self, center: Vec3, yaw: f32, pitch: f32, size: Vec3, mat: Mat) {
        self.pieces.push(Piece::Slab {
            center,
            yaw,
            size,
            mat,
            pitch,
        });
    }

    /// Turns the footprint into wall strips plus the AABB collider chain. Call once the
    /// outline is final; the `*_near` helpers below snap decor to these walls.
    pub fn build_walls(&mut self) {
        self.walls = self.fp.walls();
        let top = self.top();
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
            run(mat, 0.0, top, 0.0);
            if mat == Mat::Glass {
                run(Mat::Trim, 0.0, 0.12, 0.02);
                run(Mat::Trim, top - 0.12, top, 0.02);
            } else {
                run(Mat::Trim, 0.0, 0.2, 0.02);
                run(Mat::Accent, 2.87, 2.93, 0.02);
                self.pieces.push(Piece::Cap { segs, y: top });
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

    /// Doorway set into the nearest wall with a sign above it. Returns the arrival point 1.8 m
    /// in front and the unit yaw that faces into the room (same convention as `elevator_near`).
    pub fn door_near(&mut self, x: f32, z: f32, sign: Sign) -> (Vec3, f32) {
        let (pt, n) = self.wall_near(x, z).unwrap_or((Vec2::new(x, z), Vec2::X));
        let yaw = n.x.atan2(n.y);
        let lat = Vec2::new(yaw.cos(), -yaw.sin());
        let at = |depth: f32, l: f32, y: f32| {
            let c = pt + n * depth + lat * l;
            Vec3::new(c.x, y, c.y)
        };
        self.slab(at(0.1, 0.0, 1.3), yaw, Vec3::new(2.3, 2.6, 0.2), Mat::Dark);
        for l in [-0.5, 0.5] {
            self.slab(at(0.22, l, 1.1), yaw, Vec3::new(0.9, 2.2, 0.06), Mat::Glass);
        }
        self.slab(
            at(0.2, 0.0, 2.75),
            yaw,
            Vec3::new(1.9, 0.08, 0.06),
            Mat::LedGreen,
        );
        let c = at(0.2, 0.0, 3.25);
        self.sign(c, yaw, 1.5, sign);
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

/// Pixel size of a sign texture; `sign()` derives the panel's aspect ratio from it.
pub fn sign_canvas(sign: Sign) -> (u32, u32) {
    match sign {
        Sign::WhiteboardA | Sign::WhiteboardB | Sign::WhiteboardC => (192, 128),
        Sign::ClientName => (256, 96),
        Sign::Billboard | Sign::InnovationNeon => (256, 96),
        Sign::OffsiteWelcome | Sign::OffsiteShuttle | Sign::FamilyBanner => (256, 96),
        Sign::ObNewStarters | Sign::ObWelcome | Sign::ObNameplate => (256, 96),
        Sign::MigDoor | Sign::MigCloud | Sign::MigClock => (256, 96),
        Sign::QbrDoor | Sign::QbrWelcome | Sign::QbrSsot | Sign::QbrHockey | Sign::QbrPrize => {
            (256, 96)
        }
        Sign::QbrAgenda | Sign::QbrBuzz | Sign::QbrSynergy => (256, 128),
        Sign::H24Door
        | Sign::H24Welcome
        | Sign::H24Team
        | Sign::H24Clock
        | Sign::H24ClockNote
        | Sign::H24DemoDay
        | Sign::H24Series
        | Sign::H24Wifi
        | Sign::H24Prize
        | Sign::H24Quiet => (256, 96),
        Sign::H24Ideas | Sign::H24Sticky => (256, 128),
        Sign::MigPlan | Sign::MigTickets | Sign::MigRates | Sign::MigCutover => (256, 128),
        Sign::OffsiteAgenda | Sign::KeynoteScreen | Sign::Calendar => (256, 128),
        Sign::MarieCall => (256, 128),
        Sign::IlSplash | Sign::IlScreenA | Sign::IlScreenB | Sign::IlScreenC => (256, 96),
        Sign::IlW1Door | Sign::IlHiring | Sign::IlRackVote => (256, 96),
        Sign::IlW2Door | Sign::IlCulture1 | Sign::IlCulture2 | Sign::IlCulture3 => (256, 96),
        Sign::IlWellbeing | Sign::IlOkr | Sign::IlVoteBoard | Sign::IlGradeBoard => (256, 128),
        Sign::IlNineBox | Sign::IlFaceReview => (192, 192),
        Sign::IlW3Door
        | Sign::IlW3Back
        | Sign::IlW3ToValidation
        | Sign::IlW3ToTuning
        | Sign::IlW3Out
        | Sign::IlOptOut
        | Sign::IlTos
        | Sign::IlPieceRate
        | Sign::IlCorpus
        | Sign::IlConfident
        | Sign::IlImpossible
        | Sign::IlFineTune
        | Sign::IlBeige => (256, 96),
        Sign::IlLabelThis
        | Sign::IlRobots
        | Sign::IlTruth
        | Sign::IlTrainingLoss
        | Sign::IlStatusGreen => (256, 128),
        Sign::IlW4Door
        | Sign::IlW4Back
        | Sign::IlW4ToAutoscale
        | Sign::IlW4ToCloud
        | Sign::IlW4Out
        | Sign::IlRoofSla
        | Sign::IlFan
        | Sign::IlRackGarden
        | Sign::IlCoolingNote
        | Sign::IlReservedRack
        | Sign::IlScaleUp
        | Sign::IlEgress
        | Sign::IlFreeTier
        | Sign::IlSurprise
        | Sign::IlSomeoneElse
        | Sign::IlMultiRegion
        | Sign::IlNotBug
        | Sign::IlSla => (256, 96),
        Sign::IlLadder
        | Sign::IlBillBoard
        | Sign::IlInvoice
        | Sign::IlDashboard
        | Sign::IlRegionHub
        | Sign::IlRegionUsEast
        | Sign::IlRegionEuWest
        | Sign::IlRegionApSouth
        | Sign::IlRegionSaEast
        | Sign::IlRegionAfSouth
        | Sign::IlStatusDegraded
        | Sign::IlStatusInvest
        | Sign::IlStatusDark503
        | Sign::IlTeaser => (256, 128),
        Sign::IlFacePatch => (256, 192),
        Sign::IlPatchIdle
        | Sign::IlPatchDone
        | Sign::IlPatchMeeting
        | Sign::IlPatchDeps
        | Sign::IlPatchBreaking
        | Sign::IlPatchDeprecation
        | Sign::IlPatchPair => (256, 128),
        Sign::ResMap | Sign::ResBoard => (256, 128),
        Sign::TbPhoto | Sign::TbKaraoke | Sign::TbTrustFall => (256, 96),
        Sign::HackBanner | Sign::HackStage | Sign::MigrationNew | Sign::MigrationOld => (256, 96),
        Sign::VibeConBanner | Sign::VibeConKeynote | Sign::VibeConMasterclass => (256, 96),
        Sign::VibeConJudging | Sign::VibeConFireside | Sign::VibeConAfterparty => (256, 96),
        Sign::IlW5Door
        | Sign::IlW5Back
        | Sign::IlW5ToPolicy
        | Sign::IlW5ToRange
        | Sign::IlW5Out
        | Sign::IlPrincipleA
        | Sign::IlPrincipleB
        | Sign::IlPrincipleC
        | Sign::IlPrincipleD
        | Sign::IlHiringOne
        | Sign::IlConcerns
        | Sign::IlKiosk
        | Sign::IlTerms
        | Sign::IlAlignedBanner
        | Sign::IlSafeDefault
        | Sign::IlRedTeam
        | Sign::IlBlueTeam
        | Sign::IlRange
        | Sign::IlHoneypot
        | Sign::IlPurpleSoon
        | Sign::IlSensorLane
        | Sign::IlAlignDone
        | Sign::IlLabDark
        | Sign::IlW6Door
        | Sign::IlW6Back
        | Sign::IlW6ToExec
        | Sign::IlW6ToBoardroom
        | Sign::IlW6Out
        | Sign::IlJibRules
        | Sign::IlCraneNote
        | Sign::IlPigeon
        | Sign::IlOpenPlanTen
        | Sign::IlWellnessRoom
        | Sign::IlMassage
        | Sign::IlFruitBowl
        | Sign::IlRunway
        | Sign::IlCampusView
        | Sign::IlLiftPlate
        | Sign::IlCoreDoor
        | Sign::IlCoreWelcome
        | Sign::IlCoreMonitor
        | Sign::IlCoreRack => (256, 96),
        Sign::IlRoadmapWall => (256, 128),
        _ => (192, 96),
    }
}

pub const WALL_T: f32 = COLLIDER_T;
pub const DOOR_H: f32 = 2.5;
pub const H: f32 = WALL_HEIGHT;

/// Ceiling of the big halls (ballroom, expo, keynote, open-plan lab): the standard 4.5 m lid
/// pinned the orbit camera against the ceiling in rooms this wide.
pub const HALL_H: f32 = 6.5;

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
