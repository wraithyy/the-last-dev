//! Ilmari Wing 3, Training Data, on screen (docs/ILMARI_10.md C.3.2, C.3.3, C.4): the Hallucination's
//! hazard cycle with each player's own fakes drawn over the truth, the Ground Truth view, the
//! action recorder's ghosts, the Scrape Pit's drafts and the HUD lines that say what a fight
//! wants. Presentation only: the sim holds the truth, and nothing here feeds back into it. The
//! layout of the fakes is `combat_core::illusion::layout`, a pure function of the cycle's public
//! salt and the viewer's id, so every client draws its own and nobody else's.
// Bevy system signatures are inherently wide.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, CombatSim, SimReady, SimView};
use crate::hud::{DIM, drop_shadow, label, panel_bg, set_text};
use crate::office::Props;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use combat_core::echo::EchoKind;
use combat_core::illusion::{layout, real_set};
use combat_core::{EncounterDef, Event};
use std::f32::consts::FRAC_PI_2;

/// Ghosts on screen at once (docs/ILMARI_10.md C.7).
const GHOST_CAP: usize = 12;
/// Warning of the hazard cycle by difficulty, ms: what `Confabulate` and `Confabulate (Story)` say.
const WARN_FULL_S: f32 = 5.0;
const WARN_STORY_S: f32 = 7.0;
/// How long the landed hazards stay lit before they go.
const LANDED_S: f32 = 0.6;
const GROUNDED: &str = "il_grounded";

pub struct Wing3FxPlugin;

impl Plugin for Wing3FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Overlay>()
            .init_resource::<Drafts>()
            .add_systems(Startup, (setup, spawn_hud))
            .add_systems(
                Update,
                (
                    stage_qa,
                    cycle_events,
                    update_overlay,
                    echo_events,
                    update_ghosts,
                    draft_events,
                    update_drafts,
                    update_hud,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// `WOW_VIEWER_OFFSET=<n>`: the id added to the local player's when the fakes are derived, so
/// two screenshots of one room show two players' illusions (offline QA, never set in play).
fn qa_viewer_offset() -> u64 {
    std::env::var("WOW_VIEWER_OFFSET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// QA and bench staging, offline only: `WOW_WING3_STAGE=halluc` starts a hazard cycle round the
/// player every 9 s, `collapse` announces echo ghosts round the player every 3 s.
fn stage_qa(
    time: Res<Time>,
    mut sim: ResMut<CombatSim>,
    mut events: MessageWriter<CombatEvent>,
    mut last: Local<f32>,
) {
    let stage = match std::env::var("WOW_WING3_STAGE") {
        Ok(v) => v,
        Err(_) => match std::env::var("WOW_BENCH").as_deref() {
            Ok("ilmari3_halluc") => "halluc".into(),
            Ok("ilmari3_collapse") => "collapse".into(),
            _ => return,
        },
    };
    let now = time.elapsed_secs();
    let period = if stage == "halluc" { 9.0 } else { 3.0 };
    if now < 2.0 || now - *last < period {
        return;
    }
    *last = now;
    let Some(me) = sim.0.player() else { return };
    let Some(c) = sim.0.unit(me).map(|u| u.pos) else {
        return;
    };
    if stage == "halluc" {
        let Some(source) = sim.0.units().find(|u| u.hostile).map(|u| u.id) else {
            return;
        };
        let arena = combat_core::illusion::IllusionArena {
            x_cm: (c.x * 100.0).round() as i32,
            z_cm: ((c.z - 8.0) * 100.0).round() as i32,
            tile_cm: 800,
        };
        sim.0.illusion_begin(arena, source, 1, 8000);
    } else {
        for (k, kind) in [EchoKind::Strike, EchoKind::Hush, EchoKind::Strike]
            .into_iter()
            .enumerate()
        {
            let a = k as f32 * std::f32::consts::TAU / 3.0;
            events.write(CombatEvent(Event::Echoed {
                as_player: me,
                kind,
                at_ms: 3000,
                at_cm: [
                    ((c.x + a.cos() * 7.0) * 100.0) as i32,
                    ((c.z + a.sin() * 7.0) * 100.0) as i32,
                ],
            }));
        }
    }
}

#[derive(Resource)]
struct Assets3 {
    disc: Handle<Mesh>,
    ring: Handle<Mesh>,
    square: Handle<Mesh>,
    body: Handle<Mesh>,
    head: Handle<Mesh>,
    hazard_fill: Handle<StandardMaterial>,
    hazard_edge: Handle<StandardMaterial>,
    dim: Handle<StandardMaterial>,
    outline: Handle<StandardMaterial>,
    shimmer_a: Handle<StandardMaterial>,
    shimmer_b: Handle<StandardMaterial>,
    ghost: Handle<StandardMaterial>,
    bar_back: Handle<StandardMaterial>,
    bar_fill: Handle<StandardMaterial>,
    bar_marked: Handle<StandardMaterial>,
    lost: Handle<StandardMaterial>,
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pal: Res<crate::settings::Palette>,
) {
    let d = pal.danger;
    let mut m = |c: Color| materials.add(unlit(c));
    commands.insert_resource(Assets3 {
        disc: meshes.add(Circle::new(1.0)),
        ring: meshes.add(Annulus::new(0.9, 1.0)),
        square: meshes.add(Rectangle::new(1.0, 1.0)),
        body: meshes.add(Capsule3d::new(0.28, 1.0)),
        head: meshes.add(Sphere::new(0.22)),
        hazard_fill: m(Color::srgba(d[0], d[1], d[2], 0.3)),
        hazard_edge: m(Color::srgba(d[0], d[1], d[2], 0.9)),
        dim: m(Color::srgba(0.7, 0.7, 0.75, 0.06)),
        outline: m(Color::srgba(1.0, 1.0, 1.0, 0.95)),
        shimmer_a: m(Color::srgba(0.2, 1.0, 1.0, 0.75)),
        shimmer_b: m(Color::srgba(1.0, 0.2, 0.9, 0.75)),
        ghost: m(Color::srgba(0.75, 0.9, 1.0, 0.32)),
        bar_back: m(Color::srgba(0.04, 0.05, 0.07, 0.85)),
        bar_fill: m(Color::srgba(0.35, 0.9, 0.45, 0.95)),
        bar_marked: m(Color::srgba(0.3, 0.6, 1.0, 0.8)),
        lost: m(Color::srgba(0.9, 0.2, 0.2, 0.8)),
    });
}

/// A flat 2D mesh laid on the floor, local +Y toward -Z.
fn floor(pos: Vec3, lift: f32) -> Transform {
    Transform::from_translation(pos + Vec3::Y * lift)
        .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
}

// ------------------------------------------------------------------ the hazard cycle

/// One hazard the local player sees: real or fake (they are drawn alike unless `il_grounded`).
struct Item {
    root: Entity,
    real: bool,
    fill: Entity,
    outline: Entity,
    /// A circle's fill grows with the warning; a tile does not.
    disc: Option<f32>,
}

#[derive(Resource, Default)]
struct Overlay {
    cycle: u32,
    age: f32,
    warn: f32,
    items: Vec<Item>,
    dim: bool,
}

impl Overlay {
    fn clear(&mut self, commands: &mut Commands) {
        for it in self.items.drain(..) {
            commands.entity(it.root).despawn();
        }
        self.cycle = 0;
    }
}

/// A new cycle: the real hazards, the local player's fakes, and (Full) the tiles they believe in.
fn cycle_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    assets: Res<Assets3>,
    mut overlay: ResMut<Overlay>,
) {
    for CombatEvent(e) in events.read() {
        let Event::IllusionCycle { cycle, salt, arena } = e else {
            continue;
        };
        overlay.clear(&mut commands);
        let Some(me) = sim.player() else { continue };
        // QA only: draw another player's fakes to compare two views from one client.
        let me = me + qa_viewer_offset();
        let story = sim.raid_view().story;
        overlay.cycle = *cycle;
        overlay.age = 0.0;
        overlay.warn = if story { WARN_STORY_S } else { WARN_FULL_S };
        let real = real_set(*salt, arena);
        let mine = layout(*salt, me, *cycle, arena);
        let cm = |v: i64| v as f32 / 100.0;
        // Circles: three real, three of the viewer's own; the same look unless Story shimmers them.
        let circles = real
            .circles
            .iter()
            .map(|c| (c, true))
            .chain(mine.fakes.iter().map(|c| (c, false)));
        for (c, is_real) in circles {
            let (x, z, r) = (cm(c.x_cm), cm(c.z_cm), cm(c.r_cm));
            let root = commands
                .spawn((
                    floor(Vec3::new(x, 0.0, z), 0.05),
                    Visibility::Inherited,
                    NotShadowCaster,
                ))
                .id();
            let fill = commands
                .spawn((
                    Mesh3d(assets.disc.clone()),
                    MeshMaterial3d(assets.hazard_fill.clone()),
                    Transform::from_scale(Vec3::splat(0.02)),
                ))
                .id();
            let edge = commands
                .spawn((
                    Mesh3d(assets.ring.clone()),
                    MeshMaterial3d(assets.hazard_edge.clone()),
                    Transform::from_scale(Vec3::splat(r)),
                ))
                .id();
            let outline = commands
                .spawn((
                    Mesh3d(assets.ring.clone()),
                    MeshMaterial3d(assets.outline.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.01).with_scale(Vec3::splat(r + 0.18)),
                    Visibility::Hidden,
                ))
                .id();
            commands.entity(root).add_children(&[fill, edge, outline]);
            // Story: a fake has a colour-split edge, always.
            if story && !is_real {
                for (mat, dx) in [(&assets.shimmer_a, -0.14), (&assets.shimmer_b, 0.14)] {
                    let s = commands
                        .spawn((
                            Mesh3d(assets.ring.clone()),
                            MeshMaterial3d(mat.clone()),
                            Transform::from_xyz(dx, 0.0, 0.015).with_scale(Vec3::splat(r + 0.08)),
                        ))
                        .id();
                    commands.entity(root).add_child(s);
                }
            }
            overlay.items.push(Item {
                root,
                real: is_real,
                fill,
                outline,
                disc: Some(r),
            });
        }
        // Tiles. Full: what the viewer believes is hazard (every tile they do not think is safe);
        // some of that is real and some is not. Story: the real overflow only, never masked.
        for k in 0..combat_core::illusion::TILES {
            let truly = real.overflow >> k & 1 == 1;
            let shown = if story {
                truly
            } else {
                mine.believed_safe >> k & 1 == 0
            };
            if !shown {
                continue;
            }
            let (x, z) = arena.tile_center(k);
            let size = arena.tile_cm as f32 / 100.0 - 0.5;
            let root = commands
                .spawn((
                    floor(Vec3::new(cm(x), 0.0, cm(z)), 0.045),
                    Visibility::Inherited,
                    NotShadowCaster,
                ))
                .id();
            let fill = commands
                .spawn((
                    Mesh3d(assets.square.clone()),
                    MeshMaterial3d(assets.hazard_fill.clone()),
                    Transform::from_scale(Vec3::new(size, size, 1.0)),
                ))
                .id();
            let outline = commands
                .spawn((
                    Mesh3d(assets.ring.clone()),
                    MeshMaterial3d(assets.outline.clone()),
                    Transform::from_scale(Vec3::splat(size * 0.62)),
                    Visibility::Hidden,
                ))
                .id();
            commands.entity(root).add_children(&[fill, outline]);
            overlay.items.push(Item {
                root,
                real: truly,
                fill,
                outline,
                disc: None,
            });
        }
    }
}

/// Grows the circles with the warning, swaps fakes for the dim look and reals for an outline
/// while the local player is Grounded, and clears everything shortly after the hit lands.
fn update_overlay(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    assets: Res<Assets3>,
    mut overlay: ResMut<Overlay>,
    mut tfs: Query<&mut Transform>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    if overlay.items.is_empty() {
        return;
    }
    overlay.age += time.delta_secs();
    let age = overlay.age;
    if age > overlay.warn + LANDED_S {
        overlay.clear(&mut commands);
        return;
    }
    let grounded = sim
        .player()
        .and_then(|p| sim.unit(p))
        .is_some_and(|u| u.auras.iter().any(|a| a.key == GROUNDED));
    let k = (age / overlay.warn).clamp(0.02, 1.0);
    overlay.dim = grounded;
    for it in &overlay.items {
        if let (Some(r), Ok(mut tf)) = (it.disc, tfs.get_mut(it.fill)) {
            tf.scale = Vec3::splat(r * k);
        }
        let want = if grounded && !it.real {
            &assets.dim
        } else {
            &assets.hazard_fill
        };
        if let Ok(mut m) = mats.get_mut(it.fill)
            && m.0 != *want
        {
            m.0 = want.clone();
        }
        let show = if grounded && it.real {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Ok(mut v) = vis.get_mut(it.outline)
            && *v != show
        {
            *v = show;
        }
    }
}

// ------------------------------------------------------------------ the action recorder's ghosts

#[derive(Component)]
struct Ghost {
    age: f32,
    total: f32,
}

fn kind_color(k: EchoKind) -> Color {
    match k {
        EchoKind::Strike => Color::srgba(1.0, 0.3, 0.25, 0.85),
        EchoKind::Mend => Color::srgba(0.4, 1.0, 0.5, 0.85),
        EchoKind::Ward => Color::srgba(0.4, 0.7, 1.0, 0.85),
        EchoKind::Hush => Color::srgba(0.8, 0.4, 1.0, 0.85),
        EchoKind::Call => Color::srgba(1.0, 0.85, 0.3, 0.85),
        EchoKind::Dash => Color::srgba(1.0, 1.0, 1.0, 0.85),
    }
}

fn echo_radius(k: EchoKind) -> f32 {
    match k {
        EchoKind::Strike => 3.0,
        EchoKind::Hush => 6.0,
        _ => 1.2,
    }
}

/// `Event::Echoed`: the caster's ghost stands where the cast will land, with the spot marked for
/// the warning time. Capped at `GHOST_CAP`: the oldest goes first.
fn echo_events(
    mut commands: Commands,
    mut events: MessageReader<CombatEvent>,
    assets: Res<Assets3>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ghosts: Query<(Entity, &Ghost)>,
) {
    let mut live: Vec<(Entity, f32)> = ghosts.iter().map(|(e, g)| (e, g.age)).collect();
    for CombatEvent(e) in events.read() {
        let Event::Echoed {
            kind, at_ms, at_cm, ..
        } = e
        else {
            continue;
        };
        if live.len() >= GHOST_CAP {
            live.sort_by(|a, b| b.1.total_cmp(&a.1));
            if let Some((old, _)) = live.first().copied() {
                commands.entity(old).despawn();
                live.remove(0);
            }
        }
        let at = Vec3::new(at_cm[0] as f32 / 100.0, 0.0, at_cm[1] as f32 / 100.0);
        let tint = kind_color(*kind);
        let r = echo_radius(*kind);
        let root = commands
            .spawn((
                Ghost {
                    age: 0.0,
                    total: (*at_ms as f32 / 1000.0).max(0.2),
                },
                Transform::from_translation(at),
                Visibility::Inherited,
                NotShadowCaster,
            ))
            .id();
        let ring = commands
            .spawn((
                Mesh3d(assets.ring.clone()),
                MeshMaterial3d(materials.add(unlit(tint))),
                floor(Vec3::ZERO, 0.05).with_scale(Vec3::splat(r)),
            ))
            .id();
        let fill = commands
            .spawn((
                Mesh3d(assets.disc.clone()),
                MeshMaterial3d(materials.add(unlit(tint.with_alpha(0.16)))),
                floor(Vec3::ZERO, 0.045).with_scale(Vec3::splat(r)),
            ))
            .id();
        let body = commands
            .spawn((
                Mesh3d(assets.body.clone()),
                MeshMaterial3d(assets.ghost.clone()),
                Transform::from_xyz(0.0, 0.8, 0.0),
            ))
            .id();
        let head = commands
            .spawn((
                Mesh3d(assets.head.clone()),
                MeshMaterial3d(assets.ghost.clone()),
                Transform::from_xyz(0.0, 1.55, 0.0),
            ))
            .id();
        commands
            .entity(root)
            .add_children(&[ring, fill, body, head]);
        live.push((root, 0.0));
    }
}

fn update_ghosts(
    mut commands: Commands,
    time: Res<Time>,
    mut ghosts: Query<(Entity, &mut Ghost, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut g, mut tf) in &mut ghosts {
        g.age += dt;
        // The ghost rises slightly as the spot comes due, then pops.
        let k = (g.age / g.total).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(0.8 + 0.2 * k);
        if g.age > g.total + 0.3 {
            commands.entity(e).despawn();
        }
    }
}

// ------------------------------------------------------------------ the Scrape Pit's drafts

/// What the sim last said about the drafts: integrity (0..=1000) and Mark Confidential.
#[derive(Resource, Default)]
struct Drafts {
    integrity: Vec<u16>,
    marked: Vec<bool>,
    bars: Vec<DraftBar>,
}

struct DraftBar {
    root: Entity,
    fill: Entity,
    ring: Entity,
    key: String,
}

fn draft_events(mut events: MessageReader<CombatEvent>, mut drafts: ResMut<Drafts>) {
    for CombatEvent(e) in events.read() {
        if let Event::Drafts { integrity, marked } = e {
            drafts.integrity.clone_from(integrity);
            drafts.marked.clone_from(marked);
        }
    }
}

/// The draft prop keys of the boss in the loaded zone, in the encounter's order.
fn draft_keys(sim: &dyn crate::combat::SimRead) -> Vec<String> {
    sim.content()
        .units
        .iter()
        .find_map(|u| match &u.encounter {
            Some(EncounterDef::Drafts { props, .. }) => Some(props.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// A meter over each draft that stands: green while it is whole, shrinking as the swarm eats it,
/// with a blue ring on the floor under a Mark Confidential and a red cross on one that is gone.
/// The bars face the camera.
fn update_drafts(
    mut commands: Commands,
    sim: SimView,
    props: Res<Props>,
    assets: Res<Assets3>,
    mut drafts: ResMut<Drafts>,
    camera: Single<&GlobalTransform, With<crate::camera::OrbitCamera>>,
    mut tfs: Query<&mut Transform>,
    mut mats: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    if drafts.integrity.is_empty() {
        for b in drafts.bars.drain(..) {
            commands.entity(b.root).despawn();
        }
        return;
    }
    if drafts.bars.is_empty() {
        let keys = draft_keys(&*sim);
        for key in keys {
            let Some(spot) = props.list.iter().find(|p| p.key == key) else {
                continue;
            };
            let at = spot.center + Vec3::Y * 1.9;
            let root = commands
                .spawn((
                    Transform::from_translation(at),
                    Visibility::Inherited,
                    NotShadowCaster,
                ))
                .id();
            let back = commands
                .spawn((
                    Mesh3d(assets.square.clone()),
                    MeshMaterial3d(assets.bar_back.clone()),
                    Transform::from_scale(Vec3::new(1.5, 0.16, 1.0)),
                ))
                .id();
            let fill = commands
                .spawn((
                    Mesh3d(assets.square.clone()),
                    MeshMaterial3d(assets.bar_fill.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.01).with_scale(Vec3::new(1.44, 0.1, 1.0)),
                ))
                .id();
            let ring = commands
                .spawn((
                    Mesh3d(assets.ring.clone()),
                    MeshMaterial3d(assets.bar_marked.clone()),
                    floor(Vec3::new(spot.center.x, 0.0, spot.center.z), 0.06)
                        .with_scale(Vec3::splat(1.4)),
                    Visibility::Hidden,
                ))
                .id();
            commands.entity(root).add_children(&[back, fill]);
            commands.entity(root).add_child(ring);
            drafts.bars.push(DraftBar {
                root,
                fill,
                ring,
                key,
            });
        }
    }
    let face = camera.rotation();
    let snapshot: Vec<(u16, bool)> = (0..drafts.bars.len())
        .map(|i| {
            (
                drafts.integrity.get(i).copied().unwrap_or(0),
                drafts.marked.get(i).copied().unwrap_or(false),
            )
        })
        .collect();
    for (b, (integrity, marked)) in drafts.bars.iter().zip(snapshot) {
        if let Ok(mut tf) = tfs.get_mut(b.root) {
            tf.rotation = face;
        }
        let k = f32::from(integrity) / 1000.0;
        if let Ok(mut tf) = tfs.get_mut(b.fill) {
            tf.scale = Vec3::new((1.44 * k).max(0.02), 0.1, 1.0);
            tf.translation.x = -0.72 * (1.0 - k);
        }
        let want = if integrity == 0 {
            &assets.lost
        } else {
            &assets.bar_fill
        };
        if let Ok(mut m) = mats.get_mut(b.fill)
            && m.0 != *want
        {
            m.0 = want.clone();
        }
        let show = if marked {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Ok(mut v) = vis.get_mut(b.ring)
            && *v != show
        {
            *v = show;
        }
        let _ = &b.key;
    }
}

// ------------------------------------------------------------------ the HUD line

#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct HudText;

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: px(64),
                left: percent(50),
                width: px(520),
                margin: UiRect::left(px(-260)),
                padding: UiRect::axes(px(10), px(5)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.4, 1.0, 0.7, 0.5)),
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
        });
}

/// One line for the boss of the loaded fight: the drafts left, the cycle's warning, the training
/// loss. Nothing for any other fight.
fn update_hud(
    sim: SimView,
    drafts: Res<Drafts>,
    overlay: Res<Overlay>,
    mut root: Single<&mut Node, With<HudRoot>>,
    mut text: Single<&mut Text, With<HudText>>,
) {
    let story = sim.raid_view().story;
    let boss = |key: &str| sim.units().find(|u| u.key == key && u.alive && u.in_combat);
    let line = if !drafts.integrity.is_empty() && boss("il_scraper_queen").is_some() {
        let total = drafts.integrity.len();
        let left = drafts.integrity.iter().filter(|&&i| i > 0).count();
        let fail = sim
            .content()
            .unit_def("il_scraper_queen")
            .and_then(|d| match &d.encounter {
                Some(EncounterDef::Drafts {
                    fail_lost,
                    story_fail_lost,
                    ..
                }) => Some(if story { *story_fail_lost } else { *fail_lost }),
                _ => None,
            })
            .unwrap_or(3);
        Some(format!(
            "DRAFTS UNPUBLISHED: {left} / {total}   (lose {fail} and the Queen has learned enough)"
        ))
    } else if !overlay.items.is_empty() && boss("il_hallucination").is_some() {
        let grounded = overlay.dim;
        Some(if grounded {
            "GROUNDED: the real hazards are outlined, the fakes are dimmed".to_string()
        } else if story {
            "FAKES SHIMMER. Reals do not. Boards on the walls show the rest.".to_string()
        } else {
            "SOME OF WHAT YOU SEE IS NOT THERE. Compare notes, or read a board.".to_string()
        })
    } else if let Some(b) = boss("il_model_collapse") {
        let stacks = b
            .auras
            .iter()
            .find(|a| a.key == "il_overfitting")
            .map_or(0, |a| u32::from(a.stacks));
        let underfit = b.auras.iter().any(|a| a.key == "il_underfit");
        Some(if underfit {
            "UNDERFIT: the model is starving. It takes 20% more damage.".to_string()
        } else {
            // Cosmetic: the more it overfits, the lower the number it brags about.
            let loss = 1.0 / (1.0 + 40.0 * (stacks * stacks) as f32 + 0.5 * stacks as f32);
            format!(
                "TRAINING LOSS: {loss:.3}   {}",
                if stacks > 0 {
                    "(you are repeating yourself)"
                } else {
                    "(vary your casts)"
                }
            )
        })
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
    if let Some(l) = line {
        set_text(&mut text, &l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_match_the_authored_confabulates() {
        let c = combat_core::Content::from_ron(include_str!("../../../content/game.ron"))
            .expect("content");
        let warn = |id: u32| {
            c.spell(combat_core::SpellId(id))
                .and_then(|s| match &s.effect {
                    combat_core::Effect::Confabulate { warn_ms, .. } => {
                        Some(*warn_ms as f32 / 1000.0)
                    }
                    _ => None,
                })
        };
        assert_eq!(warn(2520), Some(WARN_FULL_S));
        assert_eq!(warn(2521), Some(WARN_STORY_S));
    }

    #[test]
    fn ghost_kinds_have_colours_and_radii() {
        for k in [
            EchoKind::Strike,
            EchoKind::Mend,
            EchoKind::Ward,
            EchoKind::Hush,
            EchoKind::Call,
            EchoKind::Dash,
        ] {
            assert!(echo_radius(k) > 0.0);
            let _ = kind_color(k);
        }
    }
}
