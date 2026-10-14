//! Playable classes on the client: the chosen class (persisted), the player's skin and kit,
//! and the "Choose your role" picker (first launch, or Esc menu > Change role).
//!
//! `WOW_CLASS=fullstack|frontend|backend|devops` skips the picker for that run;
//! `WOW_PICKER=1` opens it even when a role is stored.

use crate::anim::{Character, ModelKind};
use crate::catalog;
use crate::config::PLAYER_MODEL_SCALE;
use crate::game::RestartRun;
use crate::gear::Accessories;
use crate::hud::{
    ACCENT, ACCENT_WARM, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, vgrad,
};
use crate::input::Paused;
use crate::net::{GameMode, Lobby};
use crate::player::Player;
use crate::units::{CLASS_SKINS, spawn_skin};
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use combat_core::{Content, Role};
use std::f32::consts::PI;

/// The local player's class key (`Content::classes`).
#[derive(Resource, Clone, PartialEq, Eq, Debug)]
pub struct PlayerClass(pub String);

/// One picker card's worth of content, read once from the embedded content.
struct ClassCard {
    key: String,
    name: String,
    role: Role,
    blurb: String,
    /// Names of the level-1 spells and the bar's signature move.
    abilities: Vec<String>,
}

#[derive(Resource)]
struct Picker {
    open: bool,
    /// Opened from the Esc menu: Esc cancels, picking restarts the run.
    change: bool,
    hovered: Option<usize>,
    cards: Vec<ClassCard>,
}

/// Everything the picker spawns (UI root and stage), despawned together.
type PickerParts = Or<(With<PickerRoot>, With<PickerStage>)>;

#[derive(Component)]
struct PickerRoot;
#[derive(Component)]
struct PickerCard(usize);
#[derive(Component)]
struct PickerStage;
/// The role picker's stage camera renders into the picker UI, never the window or a shot.
#[derive(Component)]
pub struct StageCamera;
#[derive(Component)]
struct PickerFooter;
#[derive(Component)]
struct ChangeRoleMenu;
#[derive(Component)]
struct ChangeRoleButton;
#[derive(Component)]
struct ChangeRoleText;

/// The stage sits in a closed box far below every zone, so nothing else is in shot.
const STAGE: Vec3 = Vec3::new(0.0, -500.0, 0.0);
const STAGE_W: u32 = 1280;
const STAGE_H: u32 = 440;
const SPACING: f32 = 1.75;

pub fn role_color(role: Role) -> Color {
    match role {
        Role::Tank => Color::srgb(0.45, 0.62, 1.0),
        Role::Healer => Color::srgb(0.35, 0.95, 0.5),
        Role::Melee => Color::srgb(1.0, 0.45, 0.7),
        Role::Ranged => Color::srgb(1.0, 0.75, 0.3),
    }
}

/// Role tag of a player unit (by unit key), for frames and nameplates.
pub fn role_of_unit(content: &Content, unit_key: &str) -> Option<Role> {
    content.class_of_unit(unit_key).map(|c| c.role)
}

pub struct ClassesPlugin;

impl Plugin for ClassesPlugin {
    fn build(&self, app: &mut App) {
        let content = Content::from_ron(include_str!("../../../content/game.ron")).ok();
        let cards: Vec<ClassCard> = content.as_ref().map_or_else(Vec::new, cards_of);
        let known = |k: &str| cards.iter().any(|c| c.key == k);
        let env_class = std::env::var("WOW_CLASS").ok().filter(|k| known(k));
        let stored = crate::net::load_class().filter(|k| known(k));
        let class = env_class
            .clone()
            .or(stored.clone())
            .or_else(|| cards.first().map(|c| c.key.clone()))
            .unwrap_or_else(|| "fullstack".into());
        // Debug stages (lineup, bare floors) and an explicit WOW_CLASS skip the picker.
        let forced = std::env::var_os("WOW_PICKER").is_some();
        let debug_stage = std::env::var_os("WOW_LINEUP").is_some();
        let open = forced || (env_class.is_none() && stored.is_none() && !debug_stage);
        app.insert_resource(PlayerClass(class))
            .insert_resource(Picker {
                open,
                change: false,
                hovered: None,
                cards,
            })
            .add_systems(Startup, spawn_change_role_menu)
            .add_systems(PostStartup, open_on_start)
            .add_systems(
                Update,
                (
                    dress_player,
                    change_role_menu,
                    picker_input,
                    sync_picker,
                    spin_stage,
                )
                    .chain(),
            );
    }
}

fn cards_of(content: &Content) -> Vec<ClassCard> {
    content
        .classes
        .iter()
        .map(|c| {
            let spell = |id| content.spell(id).map(|s| s.name.clone());
            let mut abilities: Vec<String> = c
                .action_bar
                .iter()
                .filter(|id| content.spell(**id).is_some_and(|s| s.learn_level == 1))
                .filter_map(|id| spell(*id))
                .collect();
            // One later signature move per class makes the card say what the role is about.
            let signature = c.action_bar.get(2).and_then(|id| spell(*id)).into_iter();
            abilities.extend(signature);
            if c.resurrect != combat_core::SpellId(13) {
                abilities.extend(spell(c.resurrect));
            }
            ClassCard {
                key: c.key.clone(),
                name: c.name.clone(),
                role: c.role,
                blurb: c.blurb.clone(),
                abilities,
            }
        })
        .collect()
}

/// The player's model and kit follow the class (also after a role change).
fn dress_player(
    class: Res<PlayerClass>,
    player: Single<(Entity, &mut Character), With<Player>>,
    mut commands: Commands,
    mut done: Local<Option<String>>,
) {
    if done.as_deref() == Some(class.0.as_str()) {
        return;
    }
    *done = Some(class.0.clone());
    let unit = CLASS_SKINS
        .iter()
        .find(|(k, _)| *k == class.0)
        .map_or("senior_engineer", |(_, u)| u);
    let kind = ModelKind(catalog::unit_visual(unit).model);
    let (e, mut character) = player.into_inner();
    if character.kind != kind {
        *character = Character {
            kind,
            scale: PLAYER_MODEL_SCALE,
        };
    }
    commands.entity(e).insert(Accessories(class.0.clone()));
}

fn open_on_start(mut picker: ResMut<Picker>, mut commands: Commands, stage_assets: StageAssets) {
    if picker.open {
        picker.open = false;
        open_picker(&mut commands, &mut picker, stage_assets, false);
    }
}

/// Asset stores the picker's stage needs.
#[derive(bevy::ecs::system::SystemParam)]
struct StageAssets<'w> {
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

fn open_picker(
    commands: &mut Commands,
    picker: &mut Picker,
    mut assets: StageAssets,
    change: bool,
) {
    if picker.open {
        return;
    }
    picker.open = true;
    picker.change = change;
    picker.hovered = None;
    let image = assets.images.add(Image::new_target_texture(
        STAGE_W,
        STAGE_H,
        // WebGL2 has no texture view formats, so the target is sRGB itself with no second view.
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    spawn_stage(commands, &mut assets, image.clone(), &picker.cards);
    spawn_picker_ui(commands, picker, image);
}

fn close_picker(commands: &mut Commands, picker: &mut Picker, roots: &Query<Entity, PickerParts>) {
    picker.open = false;
    for e in roots {
        commands.entity(e).despawn();
    }
}

fn spawn_stage(
    commands: &mut Commands,
    assets: &mut StageAssets,
    image: Handle<Image>,
    cards: &[ClassCard],
) {
    let stage = commands
        .spawn((
            PickerStage,
            Transform::from_translation(STAGE),
            Visibility::default(),
        ))
        .id();
    // The camera looks slightly down at the four skins standing in a row.
    commands.spawn((
        PickerStage,
        StageCamera,
        Camera3d::default(),
        Camera {
            order: -2,
            clear_color: Color::srgb(0.03, 0.035, 0.055).into(),
            ..default()
        },
        RenderTarget::Image(image.into()),
        Transform::from_translation(STAGE + Vec3::new(0.0, 1.45, 4.6))
            .looking_at(STAGE + Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    ));
    let mut mat = |c: Color, glow: LinearRgba| {
        assets.materials.add(StandardMaterial {
            base_color: c,
            emissive: glow,
            perceptual_roughness: 0.8,
            ..default()
        })
    };
    let floor = mat(Color::srgb(0.1, 0.11, 0.15), LinearRgba::BLACK);
    let wall = mat(Color::srgb(0.06, 0.07, 0.1), LinearRgba::BLACK);
    let pad = mat(Color::srgb(0.16, 0.17, 0.22), LinearRgba::BLACK);
    let rims: Vec<Handle<StandardMaterial>> = cards
        .iter()
        .map(|c| {
            let color = role_color(c.role);
            mat(color, LinearRgba::from(color) * 4.0)
        })
        .collect();
    let floor_mesh = assets.meshes.add(Cuboid::new(30.0, 0.1, 12.0));
    let wall_mesh = assets.meshes.add(Cuboid::new(30.0, 8.0, 0.1));
    let pad_mesh = assets.meshes.add(Cylinder::new(0.62, 0.08));
    let rim_mesh = assets.meshes.add(Torus::new(0.6, 0.66));
    commands.entity(stage).with_children(|s| {
        s.spawn((
            Mesh3d(floor_mesh),
            MeshMaterial3d(floor),
            Transform::from_xyz(0.0, -0.05, 0.0),
        ));
        s.spawn((
            Mesh3d(wall_mesh),
            MeshMaterial3d(wall),
            Transform::from_xyz(0.0, 3.0, -2.0),
        ));
        for (k, rim) in rims.into_iter().enumerate() {
            let x = (k as f32 - 1.5) * SPACING;
            s.spawn((
                Mesh3d(pad_mesh.clone()),
                MeshMaterial3d(pad.clone()),
                Transform::from_xyz(x, -0.02, 0.0),
            ));
            s.spawn((
                Mesh3d(rim_mesh.clone()),
                MeshMaterial3d(rim),
                Transform::from_xyz(x, 0.01, 0.0),
            ));
        }
        // Key light from the front-left, a cool fill from the right, a rim from behind.
        for (pos, intensity, color) in [
            (
                Vec3::new(-2.5, 3.5, 3.5),
                1_200_000.0,
                Color::srgb(1.0, 0.95, 0.88),
            ),
            (
                Vec3::new(3.0, 2.5, 3.0),
                500_000.0,
                Color::srgb(0.7, 0.8, 1.0),
            ),
            (
                Vec3::new(0.0, 3.0, -1.5),
                600_000.0,
                Color::srgb(0.6, 0.85, 1.0),
            ),
        ] {
            s.spawn((
                PointLight {
                    intensity,
                    color,
                    range: 20.0,
                    ..default()
                },
                Transform::from_translation(pos),
            ));
        }
    });
    for k in 0..CLASS_SKINS.len() {
        let x = (k as f32 - 1.5) * SPACING;
        let e = spawn_skin(commands, k, STAGE + Vec3::new(x, 0.0, 0.0), PI, 0);
        commands.entity(e).insert(PickerStage);
    }
}

fn spawn_picker_ui(commands: &mut Commands, picker: &Picker, image: Handle<Image>) {
    commands
        .spawn((
            PickerRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(24), px(16)),
                row_gap: px(8),
                ..default()
            },
            GlobalZIndex(80),
            BackgroundColor(Color::srgb(0.03, 0.035, 0.055)),
        ))
        .with_children(|root| {
            root.spawn((
                label(
                    "// onboarding.rs: the AI already picked a role for you. Ignore it.",
                    13.0,
                ),
                TextColor(DIM),
            ));
            root.spawn((label("Choose your role", 34.0), TextColor(ACCENT)));
            root.spawn((
                Node {
                    width: percent(100),
                    max_width: px(1200),
                    aspect_ratio: Some(STAGE_W as f32 / STAGE_H as f32),
                    flex_shrink: 1.0,
                    min_height: px(0),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(6)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                ImageNode::new(image).with_mode(NodeImageMode::Stretch),
            ));
            root.spawn(Node {
                width: percent(100),
                max_width: px(1200),
                column_gap: px(10),
                ..default()
            })
            .with_children(|row| {
                for (i, c) in picker.cards.iter().enumerate() {
                    spawn_card(row, i, c);
                }
            });
            root.spawn((PickerFooter, label("", 13.0), TextColor(DIM)));
        });
}

fn spawn_card(row: &mut ChildSpawnerCommands, i: usize, c: &ClassCard) {
    let color = role_color(c.role);
    row.spawn((
        Button,
        PickerCard(i),
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            padding: UiRect::all(px(10)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
    ))
    .with_children(|card| {
        card.spawn(Node {
            column_gap: px(8),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|head| {
            head.spawn((label(format!("[{}]", i + 1), 13.0), TextColor(DIM)));
            head.spawn((
                Node {
                    padding: UiRect::axes(px(5), px(1)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(color),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                children![(label(c.role.tag(), 11.0), TextColor(color))],
            ));
        });
        card.spawn((label(c.name.clone(), 19.0), TextColor(Color::WHITE)));
        card.spawn((
            label(c.blurb.clone(), 12.0),
            TextColor(Color::srgb(0.85, 0.88, 0.95)),
        ));
        card.spawn((
            label(format!("abilities: {}", c.abilities.join(", ")), 11.0),
            TextColor(ACCENT),
        ));
    });
}

/// Esc menu entry under the co-op button; a change restarts the solo run.
fn spawn_change_role_menu(mut commands: Commands) {
    commands.spawn((
        ChangeRoleMenu,
        Node {
            position_type: PositionType::Absolute,
            top: percent(81),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![
            (
                Button,
                ChangeRoleButton,
                Node {
                    padding: UiRect::axes(px(14), px(5)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![label("Change role", 16.0)],
            ),
            (ChangeRoleText, label("", 12.0), TextColor(DIM)),
        ],
    ));
}

#[allow(clippy::too_many_arguments)] // Bevy system: one parameter per resource it touches
fn change_role_menu(
    mut commands: Commands,
    paused: Res<Paused>,
    lobby: Res<Lobby>,
    mode: Res<GameMode>,
    mut picker: ResMut<Picker>,
    stage_assets: StageAssets,
    mut menu: Single<&mut Node, With<ChangeRoleMenu>>,
    // A Query, not Single: a Single with Changed fails while idle and skips the whole system.
    button: Query<&Interaction, (Changed<Interaction>, With<ChangeRoleButton>)>,
    mut hint: Single<&mut Text, With<ChangeRoleText>>,
) {
    let show = paused.0 && !lobby.open && !picker.open;
    let want = if show { Display::Flex } else { Display::None };
    if menu.display != want {
        menu.display = want;
    }
    let online = mode.is_online();
    let text = if online {
        "Leave the room first: roles change offline only."
    } else {
        "Your progress restarts. Like every reorg."
    };
    if hint.0 != text {
        hint.0 = text.into();
    }
    if show && !online && button.iter().any(|i| *i == Interaction::Pressed) {
        open_picker(&mut commands, &mut picker, stage_assets, true);
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system: one parameter per resource it touches
fn picker_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut picker: ResMut<Picker>,
    mut paused: ResMut<Paused>,
    mut class: ResMut<PlayerClass>,
    mode: Res<GameMode>,
    cards: Query<(&Interaction, &PickerCard)>,
    roots: Query<Entity, PickerParts>,
    mut restart: MessageWriter<RestartRun>,
) {
    if !picker.open {
        return;
    }
    // The run waits underneath; Esc (handled in targeting.rs) must not unpause it.
    paused.0 = true;
    if picker.change && keys.just_pressed(KeyCode::Escape) {
        close_picker(&mut commands, &mut picker, &roots);
        return;
    }
    picker.hovered = cards
        .iter()
        .find(|(i, _)| **i != Interaction::None)
        .map(|(_, c)| c.0);
    let digit = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .iter()
    .position(|k| keys.just_pressed(*k));
    let clicked = cards
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| c.0);
    let Some(pick) = clicked.or(digit) else {
        return;
    };
    let Some(key) = picker.cards.get(pick).map(|c| c.key.clone()) else {
        return;
    };
    let changed = class.0 != key;
    crate::net::save_class(&key);
    if changed {
        class.0 = key;
    }
    let change = picker.change;
    close_picker(&mut commands, &mut picker, &roots);
    paused.0 = false;
    // The default-class sim already runs underneath on first launch; any other pick (or any
    // pick from the menu) starts over as the new role.
    if (changed || change) && !mode.is_online() {
        restart.write(RestartRun);
    }
}

fn sync_picker(
    picker: Res<Picker>,
    mut cards: Query<(&PickerCard, &mut BorderColor)>,
    mut footer: Query<&mut Text, With<PickerFooter>>,
) {
    if !picker.open {
        return;
    }
    for (c, mut border) in &mut cards {
        let color = match picker.hovered {
            Some(h) if h == c.0 => ACCENT_WARM,
            _ => KEY_RIM,
        };
        border.set_all(color);
    }
    let text = if picker.change {
        "1-4 or click to pick. Esc cancels. Picking restarts your progress at level 1."
    } else {
        "1-4 or click to pick. You can change it later from the Esc menu (progress restarts)."
    };
    for mut t in &mut footer {
        if t.0 != text {
            t.0 = text.into();
        }
    }
}

/// The hovered card's skin turns to show itself off; the rest face the camera.
fn spin_stage(
    time: Res<Time>,
    picker: Res<Picker>,
    mut skins: Query<(&Accessories, &mut Transform), With<PickerStage>>,
) {
    if !picker.open {
        return;
    }
    let hovered = picker
        .hovered
        .and_then(|h| picker.cards.get(h))
        .map(|c| c.key.as_str());
    for (kit, mut tf) in &mut skins {
        let yaw = if Some(kit.0.as_str()) == hovered {
            PI + (time.elapsed_secs() * 1.6).sin() * 0.6
        } else {
            PI
        };
        tf.rotation = Quat::from_rotation_y(yaw);
    }
}
