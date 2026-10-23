//! Ilmari Wing 4, The Cloud, on screen (docs/ILMARI_10.md C.3.4, C.3.5, C.5): the cloud bill's bar,
//! the Region Outage's platforms with their state colours and warnings, the status pylons on the
//! hub that swap from green to orange to red, and the HUD line that says which region is next.
//! Presentation only: the sim holds the truth (`wing4.rs`) and nothing here feeds back into it.
//! Platforms are drawn from `ZoneDef::platforms`, the same layout the sim reads, as flat unlit
//! decals over the floor the room already has, so a platform that goes dark is a black disc with
//! a red "503" on it and the floor never changes under anyone.
// Bevy system signatures are inherently wide.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimView};
use crate::floors::{Sign, sign_image};
use crate::game::{GameFlow, ZoneDecor};
use crate::hud::{DIM, drop_shadow, label, panel_bg, set_text};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use combat_core::{EncounterDef, Event, PlatformKind, PlatformPhase, PlatformShape};
use std::f32::consts::FRAC_PI_2;

const ZONE: &str = "ilmari4/3";
const BOSS: &str = "il_region_outage";
const SCALER: &str = "il_autoscaler";

pub struct Wing4FxPlugin;

impl Plugin for Wing4FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cloud>()
            .init_resource::<Sequence>()
            .add_systems(Startup, (setup, spawn_hud))
            .add_systems(
                Update,
                (
                    stage_qa,
                    bill_events,
                    platform_events,
                    build_platforms,
                    update_platforms,
                    update_pylons,
                    teaser,
                    update_hud,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// QA and bench staging, offline only. `WOW_WING4_STAGE=outage` pulls the Region Outage on the
/// first frames (the player cannot die, so the sequence plays for the camera); `bill` pulls the
/// Autoscaler and splits it twice (`WOW_BILL=<points>` sets the meter), `bill1` leaves it whole.
fn stage_qa(
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    mut step: Local<u8>,
    mut last: Local<f32>,
) {
    let stage = match std::env::var("WOW_WING4_STAGE") {
        Ok(v) => v,
        Err(_) => match std::env::var("WOW_BENCH").as_deref() {
            Ok("ilmari4_autoscaler") => "bill".into(),
            Ok("ilmari4_outage") => "outage".into(),
            _ => return,
        },
    };
    let Some(me) = sim.0.player() else { return };
    // Nobody dies for a screenshot.
    sim.0.debug_set_hp(me, u32::MAX);
    let now = time.elapsed_secs();
    if now < 1.5 {
        return;
    }
    let boss_key = if stage == "outage" { BOSS } else { SCALER };
    let Some((boss, hp)) = sim
        .0
        .units()
        .find(|u| u.key == boss_key && u.alive)
        .map(|u| (u.id, u.max_hp))
    else {
        return;
    };
    match *step {
        0 => {
            // The pull: a hit from the player is enough to start the fight and arm its machinery.
            sim.0.debug_hit(me, boss, 1);
            *step = 1;
            *last = now;
        }
        1 if stage == "bill" || stage == "bill1" => {
            if stage == "bill" && now - *last > 1.5 {
                // Two bursts a few seconds apart: three replicas on the floor.
                for _ in 0..2 {
                    let first = sim
                        .0
                        .units()
                        .find(|u| u.key == SCALER && u.alive)
                        .map(|u| u.id);
                    if let Some(f) = first {
                        sim.0.debug_set_hp(f, hp);
                        sim.0.debug_hit(me, f, hp / 5);
                    }
                }
                *step = 2;
                *last = now;
            }
        }
        2 if stage == "bill" && now - *last > 7.0 => {
            let first = sim
                .0
                .units()
                .find(|u| u.key == SCALER && u.alive)
                .map(|u| u.id);
            if let Some(f) = first {
                sim.0.debug_set_hp(f, hp);
                sim.0.debug_hit(me, f, hp / 5);
            }
            let want = std::env::var("WOW_BILL").ok().and_then(|v| v.parse().ok());
            if let Some(points) = want {
                sim.0.debug_set_cloud_bill(points);
            }
            *step = 3;
        }
        3 => {
            let want = std::env::var("WOW_BILL").ok().and_then(|v| v.parse().ok());
            if let Some(points) = want
                && sim.0.cloud_bill().is_some_and(|b| b < points)
            {
                sim.0.debug_set_cloud_bill(points);
            }
        }
        _ => {}
    }
    // The boss stays up for the whole shoot.
    let low = sim
        .0
        .units()
        .find(|u| u.id == boss)
        .filter(|u| u.hp * 10 < u.max_hp)
        .map(|u| u.max_hp);
    if let Some(max) = low {
        sim.0.debug_set_hp(boss, max);
    }
}

// ------------------------------------------------------------------ the cloud bill

/// What the sim last said about the bill; `active` while the Autoscaler's fight is armed.
#[derive(Resource, Default)]
struct Cloud {
    active: bool,
    bill: u16,
    copies: u8,
    reserved: bool,
}

fn bill_events(mut events: MessageReader<CombatEvent>, mut cloud: ResMut<Cloud>) {
    for CombatEvent(e) in events.read() {
        if let Event::CloudBill {
            bill,
            copies,
            reserved,
        } = e
        {
            // 0 copies is the fight ending; 1 copy and 0 points is it starting.
            cloud.active = *copies > 0;
            cloud.bill = *bill;
            cloud.copies = *copies;
            cloud.reserved = *reserved;
        }
    }
}

// ------------------------------------------------------------------ platforms

/// What the sim last said about the platforms, and what is drawn for them.
#[derive(Resource, Default)]
struct Sequence {
    phase: Vec<(PlatformPhase, f32)>,
    /// The region the sim warned about, with the sim ms it degrades at.
    warn: Option<(usize, u64)>,
    /// Story: the whole tour, next first (platform indices).
    order: Vec<usize>,
    built: bool,
    items: Vec<Item>,
    pylons: Vec<Entity>,
}

struct Item {
    idx: usize,
    fill: Entity,
    rim: Entity,
    label: Entity,
    code: Entity,
}

fn platform_events(
    mut events: MessageReader<CombatEvent>,
    time: Res<Time>,
    mut seq: ResMut<Sequence>,
) {
    let now = time.elapsed_secs();
    for CombatEvent(e) in events.read() {
        match e {
            Event::PlatformState { idx, state, .. } => {
                let k = usize::from(*idx);
                if seq.phase.len() <= k {
                    seq.phase.resize(k + 1, (PlatformPhase::Live, now));
                }
                seq.phase[k] = (*state, now);
                if *state == PlatformPhase::Degraded && seq.warn.is_some_and(|w| w.0 == k) {
                    seq.warn = None;
                }
            }
            Event::PlatformWarn { idx, at_ms, order } => {
                seq.warn = (*idx != u8::MAX).then_some((usize::from(*idx), *at_ms));
                seq.order = order.iter().map(|&i| usize::from(i)).collect();
            }
            _ => {}
        }
    }
}

#[derive(Resource)]
struct Assets4 {
    disc: Handle<Mesh>,
    ring: Handle<Mesh>,
    square: Handle<Mesh>,
    live_rim: Handle<StandardMaterial>,
    warn_rim: Handle<StandardMaterial>,
    degraded: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    dark_rim: Handle<StandardMaterial>,
    restoring: Handle<StandardMaterial>,
    clear: Handle<StandardMaterial>,
    code: Handle<StandardMaterial>,
    labels: Vec<(String, Handle<StandardMaterial>)>,
    screens: [Handle<StandardMaterial>; 3],
    teaser: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    coat: Handle<StandardMaterial>,
}

fn unlit(c: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: c,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

fn m_unlit(materials: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    materials.add(unlit(c))
}

fn textured(img: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(img),
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut m = |c: Color| materials.add(unlit(c));
    let live_rim = m(Color::srgba(0.35, 1.0, 0.6, 0.55));
    let warn_rim = m(Color::srgba(1.0, 0.85, 0.2, 0.9));
    let degraded = m(Color::srgba(1.0, 0.55, 0.1, 0.6));
    let dark = m(Color::srgba(0.01, 0.01, 0.03, 0.9));
    let dark_rim = m(Color::srgba(1.0, 0.15, 0.1, 0.95));
    let restoring = m(Color::srgba(0.4, 0.7, 1.0, 0.4));
    let clear = m(Color::srgba(0.0, 0.0, 0.0, 0.0));
    let mut tex = |sign: Sign| materials.add(textured(images.add(sign_image(sign))));
    let code = tex(Sign::IlStatusDark503);
    let labels = [
        ("us-east-1", Sign::IlRegionUsEast),
        ("eu-west-1", Sign::IlRegionEuWest),
        ("ap-south-1", Sign::IlRegionApSouth),
        ("sa-east-1", Sign::IlRegionSaEast),
        ("af-south-1", Sign::IlRegionAfSouth),
        ("hub", Sign::IlRegionHub),
    ]
    .map(|(id, s)| (id.to_string(), tex(s)))
    .to_vec();
    let screens = [
        tex(Sign::IlStatusGreen),
        tex(Sign::IlStatusDegraded),
        tex(Sign::IlStatusInvest),
    ];
    let teaser = tex(Sign::IlTeaser);
    let glow = m_unlit(&mut materials, Color::srgba(0.75, 0.45, 1.0, 0.85));
    let coat = m_unlit(&mut materials, Color::srgba(0.97, 0.97, 1.0, 0.95));
    commands.insert_resource(Assets4 {
        disc: meshes.add(Circle::new(1.0)),
        ring: meshes.add(Annulus::new(0.93, 1.0)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        live_rim,
        warn_rim,
        degraded,
        dark,
        dark_rim,
        restoring,
        clear,
        code,
        labels,
        screens,
        teaser,
        glow,
        coat,
    });
}

/// A flat 2D mesh laid on the floor.
fn floor(pos: Vec3, lift: f32) -> Transform {
    Transform::from_translation(pos + Vec3::Y * lift)
        .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
}

/// The hub's world position: the boss's spawn point, which the sim uses as the platforms' origin.
fn origin(flow: &GameFlow) -> Option<Vec3> {
    flow.anchors.iter().find(|a| a.0 == "boss").map(|a| a.1)
}

/// Draws one decal set per region and the hub of the Server Cloud, once per load of the zone;
/// removes them (and the pylon screens) when the zone changes, through `ZoneDecor`.
fn build_platforms(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    assets: Res<Assets4>,
    mut seq: ResMut<Sequence>,
) {
    if flow.zone != ZONE {
        if seq.built {
            *seq = Sequence::default();
        }
        return;
    }
    if seq.built {
        return;
    }
    let (Some(at), Some(z)) = (origin(&flow), sim.content().zone(ZONE)) else {
        return;
    };
    seq.built = true;
    for (idx, d) in z.platforms.iter().enumerate() {
        if d.kind == PlatformKind::Bridge {
            continue;
        }
        let PlatformShape::Disc { r } = d.shape else {
            continue;
        };
        let c = Vec3::new(at.x + d.x, 0.0, at.z + d.z);
        let fill = commands
            .spawn((
                Mesh3d(assets.disc.clone()),
                MeshMaterial3d(assets.clear.clone()),
                floor(c, 0.07).with_scale(Vec3::splat(r)),
                NotShadowCaster,
                ZoneDecor,
            ))
            .id();
        let rim = commands
            .spawn((
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(assets.live_rim.clone()),
                floor(c, 0.09).with_scale(Vec3::splat(r)),
                NotShadowCaster,
                ZoneDecor,
            ))
            .id();
        // The region's name, big, on the floor of its platform.
        let lbl = assets
            .labels
            .iter()
            .find(|(id, _)| *id == d.id)
            .map(|(_, m)| m.clone());
        let label = commands
            .spawn((
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(lbl.unwrap_or_else(|| assets.clear.clone())),
                floor(c, 0.08).with_scale(Vec3::new(r * 1.3, r * 0.65, 1.0)),
                NotShadowCaster,
                ZoneDecor,
            ))
            .id();
        let code = commands
            .spawn((
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(assets.code.clone()),
                floor(c, 0.1).with_scale(Vec3::new(r * 1.5, r * 0.75, 1.0)),
                Visibility::Hidden,
                NotShadowCaster,
                ZoneDecor,
            ))
            .id();
        seq.items.push(Item {
            idx,
            fill,
            rim,
            label,
            code,
        });
    }
    // The pylon screens on the hub: dead frames are in the layout, the live faces are here.
    for (dx, dz, yaw) in crate::floors::status_pylons() {
        let pos = Vec3::new(at.x + dx, 3.1, at.z + dz);
        let screen = commands
            .spawn((
                Mesh3d(assets.square.clone()),
                MeshMaterial3d(assets.screens[0].clone()),
                Transform::from_translation(pos + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 0.14)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::new(2.9, 1.6, 1.0)),
                NotShadowCaster,
                ZoneDecor,
            ))
            .id();
        seq.pylons.push(screen);
    }
}

/// The decal state of every platform from the last word of the sim.
fn update_platforms(
    time: Res<Time>,
    assets: Res<Assets4>,
    seq: Res<Sequence>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    if !seq.built || seq.items.is_empty() {
        return;
    }
    let t = time.elapsed_secs();
    let pulse = 0.5 + 0.5 * (t * 6.0).sin();
    // Two shared materials breathe; everything else is a fixed handle.
    if let Some(mut m) = materials.get_mut(&assets.degraded) {
        m.base_color = Color::srgba(1.0, 0.55, 0.1, 0.5 + 0.3 * pulse);
    }
    if let Some(mut m) = materials.get_mut(&assets.warn_rim) {
        m.base_color = Color::srgba(1.0, 0.85, 0.2, 0.45 + 0.5 * pulse);
    }
    let flick = if (t * 14.0).sin() > 0.2 { 0.55 } else { 0.1 };
    if let Some(mut m) = materials.get_mut(&assets.restoring) {
        m.base_color = Color::srgba(0.4, 0.7, 1.0, flick);
    }
    let warned = seq.warn.map(|w| w.0);
    for it in &seq.items {
        let phase = seq.phase.get(it.idx).map_or(PlatformPhase::Live, |p| p.0);
        let (fill, rim, code) = match phase {
            PlatformPhase::Live if warned == Some(it.idx) => {
                (&assets.clear, &assets.warn_rim, false)
            }
            PlatformPhase::Live => (&assets.clear, &assets.live_rim, false),
            PlatformPhase::Degraded => (&assets.degraded, &assets.warn_rim, false),
            PlatformPhase::Dark => (&assets.dark, &assets.dark_rim, true),
            PlatformPhase::Restoring => (&assets.restoring, &assets.live_rim, false),
        };
        for (e, want) in [(it.fill, fill), (it.rim, rim)] {
            if let Ok(mut m) = mats.get_mut(e)
                && m.0 != *want
            {
                m.0 = want.clone();
            }
        }
        let show = |on: bool| {
            if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            }
        };
        if let Ok(mut v) = vis.get_mut(it.code) {
            *v = show(code);
        }
        // The name is read from afar while the platform is up and gets out of the way of the 503.
        if let Ok(mut v) = vis.get_mut(it.label) {
            *v = show(!code);
        }
    }
}

/// The pylons' screens: green while the sequence is quiet, orange while a region degrades, red
/// ("Investigating") while one is dark. Deliberately wrong: the room is on fire either way.
fn update_pylons(
    assets: Res<Assets4>,
    seq: Res<Sequence>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    if seq.pylons.is_empty() {
        return;
    }
    let worst = seq.phase.iter().fold(0, |w, p| {
        w.max(match p.0 {
            PlatformPhase::Dark => 2,
            PlatformPhase::Degraded => 1,
            _ => 0,
        })
    });
    for &e in &seq.pylons {
        if let Ok(mut m) = mats.get_mut(e)
            && m.0 != assets.screens[worst]
        {
            m.0 = assets.screens[worst].clone();
        }
    }
}

// ------------------------------------------------------------------ the door on the roof

/// Once the wing is done (the quest turned in, or the Region Outage down this week) a door with
/// a lab coat on it lights up on the Cooling Deck's north rail. It says "coming" and nothing else:
/// the 1.0 core is not in this version.
fn teaser(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    assets: Res<Assets4>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut done: Local<bool>,
) {
    if flow.zone != "ilmari4/1" {
        *done = false;
        return;
    }
    if *done {
        return;
    }
    let view = sim.raid_view();
    let cleared = sim.quest_status("il_w4_done") == combat_core::QuestStatus::Done
        || view.locked.contains(&view.key(BOSS));
    if !cleared {
        return;
    }
    *done = true;
    let (x, z) = (14.0, -23.6);
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let mut bit = |m: &Handle<StandardMaterial>, p: Vec3, s: Vec3| {
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(m.clone()),
            Transform::from_translation(p).with_scale(s),
            NotShadowCaster,
            ZoneDecor,
        ));
    };
    // The door: a lit frame round a violet leaf, and the sign.
    bit(
        &assets.glow,
        Vec3::new(x, 1.2, z + 0.05),
        Vec3::new(1.5, 2.5, 1.0),
    );
    bit(
        &assets.clear,
        Vec3::new(x, 1.1, z + 0.06),
        Vec3::new(1.1, 2.2, 1.0),
    );
    bit(
        &assets.teaser,
        Vec3::new(x, 3.0, z + 0.05),
        Vec3::new(2.6, 1.3, 1.0),
    );
    // The coat: a torso, two sleeves and a pale collar.
    bit(
        &assets.coat,
        Vec3::new(x, 1.2, z + 0.08),
        Vec3::new(0.55, 0.9, 1.0),
    );
    bit(
        &assets.coat,
        Vec3::new(x - 0.42, 1.2, z + 0.08),
        Vec3::new(0.25, 0.8, 1.0),
    );
    bit(
        &assets.coat,
        Vec3::new(x + 0.42, 1.2, z + 0.08),
        Vec3::new(0.25, 0.8, 1.0),
    );
    bit(
        &assets.glow,
        Vec3::new(x, 1.55, z + 0.09),
        Vec3::new(0.16, 0.3, 1.0),
    );
}

// ------------------------------------------------------------------ the HUD

#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct HudText;
#[derive(Component)]
struct HudBar;
#[derive(Component)]
struct HudFill;

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(64),
                left: percent(50),
                width: px(560),
                margin: UiRect::left(px(-280)),
                padding: UiRect::axes(px(10), px(5)),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 0.7, 0.2, 0.6)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(13),
        ))
        .with_children(|c| {
            c.spawn((
                HudText,
                label("", 13.0),
                TextColor(DIM),
                TextLayout::justify(Justify::Center),
            ));
            c.spawn((
                HudBar,
                Node {
                    width: percent(100),
                    height: px(9),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.9)),
            ))
            .with_children(|b| {
                b.spawn((
                    HudFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.9, 0.45)),
                ));
                // The Overage steps at a quarter, a half and three quarters.
                for k in 1..4 {
                    b.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(25.0 * k as f32),
                            width: px(1),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                    ));
                }
            });
        });
}

/// Seconds until the bill reaches 1000 at its present rate (`None`: never, it is not climbing).
fn forecast_s(sim: &dyn crate::combat::SimRead, cloud: &Cloud) -> Option<u32> {
    let story = sim.raid_view().story;
    let rate = sim
        .content()
        .unit_def(SCALER)
        .and_then(|d| match &d.encounter {
            Some(EncounterDef::Cloud(b)) => {
                let i = usize::from(cloud.copies.max(1) - 1).min(b.rate_tenths.len() - 1);
                let mut r = b.rate_tenths[i];
                if story {
                    r = r * b.story_rate_pct / 100;
                }
                if cloud.reserved {
                    r /= 2;
                }
                Some(r)
            }
            _ => None,
        })?;
    (rate > 0).then(|| (u32::from(1000 - cloud.bill.min(1000)) * 10).div_ceil(rate))
}

/// One panel for either fight: the bill (Autoscaler) or the next region (Region Outage).
fn update_hud(
    sim: SimView,
    cloud: Res<Cloud>,
    seq: Res<Sequence>,
    flow: Res<GameFlow>,
    mut root: Single<&mut Node, (With<HudRoot>, Without<HudBar>, Without<HudFill>)>,
    mut text: Single<&mut Text, With<HudText>>,
    mut bar: Single<&mut Node, (With<HudBar>, Without<HudRoot>, Without<HudFill>)>,
    mut fill: Single<
        (&mut Node, &mut BackgroundColor),
        (With<HudFill>, Without<HudRoot>, Without<HudBar>),
    >,
) {
    let story = sim.raid_view().story;
    let scaler_up = cloud.active
        && sim
            .units()
            .any(|u| u.key == SCALER && u.alive && u.in_combat);
    let outage = flow.zone == ZONE
        && seq.built
        && sim.units().any(|u| u.key == BOSS && u.alive && u.in_combat);
    let mut show_bar = false;
    let line = if scaler_up {
        show_bar = true;
        let points = f32::from(cloud.bill);
        let money = points * 4096.0;
        let name = if cloud.bill >= 1000 {
            "BUDGET EXCEEDED"
        } else if cloud.bill >= 750 {
            "OVERAGE III"
        } else if cloud.bill >= 500 {
            "OVERAGE II"
        } else if cloud.bill >= 250 {
            "OVERAGE I"
        } else {
            "APPROVAL REQUIRED"
        };
        let eta = forecast_s(&*sim, &cloud)
            .filter(|_| cloud.bill < 1000)
            .map_or(String::new(), |s| format!("   Budget Exceeded in {s} s"));
        let rack = if cloud.reserved {
            "   RESERVED: HALF RATE"
        } else {
            ""
        };
        fill.0.width = percent(points / 10.0);
        fill.1.0 = if cloud.bill >= 750 {
            Color::srgb(1.0, 0.25, 0.2)
        } else if cloud.bill >= 400 {
            Color::srgb(1.0, 0.7, 0.15)
        } else {
            Color::srgb(0.35, 0.9, 0.45)
        };
        Some(format!(
            "CLOUD BILL: ${money:>9.2}  [{name}]   Copies: {}{eta}{rack}",
            cloud.copies
        ))
    } else if outage {
        let now_ms = combat_core::ticks_to_ms(sim.now().0);
        let label_of = |idx: usize| -> String {
            sim.content()
                .zone(ZONE)
                .and_then(|z| z.platforms.get(idx))
                .map_or_else(|| "?".to_string(), |d| d.label.clone())
        };
        let dark: Vec<String> = seq
            .phase
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.0, PlatformPhase::Dark | PlatformPhase::Degraded))
            .map(|(i, _)| label_of(i))
            .collect();
        let next = seq.warn.map(|(idx, at)| {
            let s = at.saturating_sub(now_ms) / 1000;
            format!("NEXT: {} in {s} s", label_of(idx))
        });
        let tour = if story && !seq.order.is_empty() {
            let names: Vec<String> = seq.order.iter().map(|&i| label_of(i)).collect();
            format!("\nORDER: {}", names.join(" > "))
        } else {
            String::new()
        };
        let down = if dark.is_empty() {
            "All Systems Operational".to_string()
        } else {
            format!(
                "DOWN: {}   (stand on a lit platform or the hub)",
                dark.join(", ")
            )
        };
        Some(format!(
            "{down}   {}{tour}",
            next.unwrap_or_else(|| "NEXT: investigating".to_string())
        ))
    } else {
        None
    };
    let display = if line.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != display {
        root.display = display;
    }
    let bar_display = if show_bar {
        Display::Flex
    } else {
        Display::None
    };
    if bar.display != bar_display {
        bar.display = bar_display;
    }
    if let Some(l) = line {
        set_text(&mut text, &l);
    }
}
