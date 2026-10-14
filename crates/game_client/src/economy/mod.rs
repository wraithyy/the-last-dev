//! Client side of the economy: the `~/whoami` character sheet (equipment, stats, bag), the
//! `~/procurement` vendor window, the gadget bar for on-use gear, item tooltips, gear worn
//! visibly on every player model, loot beams, and the offline save.
//!
//! Debug and screenshot switches:
//! - `WOW_GEAR=<item>|<item>..`: equip these pieces on the local player at start (offline).
//! - `WOW_SP=<n>`: start with that many Story Points; `WOW_BAG=<item>|..` puts goods in the bag.
//! - `WOW_SHEET=1`: open the character sheet; `WOW_SHOP=1`: open the nearest vendor two seconds in.
//! - `WOW_HOVER_ITEM=<item>`: pin that item's tooltip (screenshots of comparisons).
//! - `WOW_LINEUP_GEAR=1`: lineup skins (`WOW_LINEUP=4`) wear a sample outfit per class.
//! - `WOW_NO_SAVE=1`: neither load nor write the offline save.
//! - `WOW_LOOT=<secs>`: loot the nearest corpse within reach at that time (loot window shots).

// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod goods;
mod outfit;
mod save;
mod sheet;
mod shop;

use crate::combat::{CombatSim, SimReady, SimView};
use crate::hud::{ACCENT, PANEL_EDGE, drop_shadow, label, panel_bg};
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use combat_core::EntityId;

/// Which economy windows are open, and the purchase waiting for a second click.
#[derive(Resource, Default)]
pub struct EconomyUi {
    pub sheet: bool,
    /// The vendor (sim id) whose shop is open.
    pub shop: Option<EntityId>,
    /// Expensive purchase awaiting confirmation: (item, seconds left).
    pub confirm: Option<(String, f32)>,
}

/// The item under the cursor this frame (any economy window sets it); the tooltip follows it.
#[derive(Resource, Default)]
pub struct HoverItem(pub Option<String>);

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EconomyUi>()
            .init_resource::<HoverItem>()
            .add_systems(PreUpdate, close_on_escape.after(InputSystems))
            .add_systems(Startup, spawn_tooltip)
            .add_systems(PostStartup, debug_start.in_set(SimReady))
            .add_systems(Update, (update_tooltip, debug_loot).in_set(SimReady))
            .add_systems(Update, pin_ui_camera)
            .add_plugins((
                sheet::SheetPlugin,
                shop::ShopPlugin,
                outfit::OutfitPlugin,
                save::SavePlugin,
            ));
    }
}

/// Esc closes the shop, then the sheet, and is swallowed so it neither pauses nor clears the
/// target in the same press.
fn close_on_escape(mut keys: ResMut<ButtonInput<KeyCode>>, mut ui: ResMut<EconomyUi>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if ui.shop.take().is_some() || std::mem::take(&mut ui.sheet) {
        ui.confirm = None;
        keys.clear_just_pressed(KeyCode::Escape);
    }
}

fn env_list(name: &str) -> Vec<String> {
    std::env::var(name)
        .map(|v| {
            v.split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn debug_start(mut sim: ResMut<CombatSim>, mut ui: ResMut<EconomyUi>) {
    let Some(p) = sim.0.player() else { return };
    for item in env_list("WOW_GEAR") {
        if !sim.0.debug_equip(p, &item) {
            warn!("WOW_GEAR: unknown gear {item:?}");
        }
    }
    for item in env_list("WOW_BAG") {
        if !sim.0.grant_goods(p, &item) {
            warn!("WOW_BAG: unknown goods {item:?}");
        }
    }
    if let Some(sp) = std::env::var("WOW_SP").ok().and_then(|v| v.parse().ok()) {
        sim.0.grant_sp(p, sp);
    }
    ui.sheet |= std::env::var_os("WOW_SHEET").is_some();
}

/// The sheet's preview is a second 3D camera; the HUD must stay on the play camera.
fn pin_ui_camera(
    mut commands: Commands,
    cams: Query<
        Entity,
        (
            Added<crate::camera::OrbitCamera>,
            Without<IsDefaultUiCamera>,
        ),
    >,
) {
    for e in &cams {
        commands.entity(e).insert(IsDefaultUiCamera);
    }
}

fn debug_loot(
    time: Res<Time>,
    sim: SimView,
    mut pending: ResMut<crate::combat::PendingCommands>,
    mut done: Local<bool>,
) {
    let Some(at) = std::env::var("WOW_LOOT")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    else {
        return;
    };
    if *done || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    let Some(me) = sim.player().and_then(|p| sim.unit(p)).cloned() else {
        return;
    };
    let reach = sim.content().config.loot_range_m;
    if let Some(c) = sim
        .units()
        .find(|u| u.lootable && combat_core::distance(u.pos, me.pos) <= reach)
    {
        pending.0.push(combat_core::Command::Loot {
            looter: me.id,
            corpse: c.id,
        });
    }
}

#[derive(Component)]
struct Tooltip;

fn spawn_tooltip(mut commands: Commands) {
    commands.spawn((
        Tooltip,
        Node {
            position_type: PositionType::Absolute,
            width: px(330),
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(PANEL_EDGE),
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(60),
    ));
}

fn update_tooltip(
    mut commands: Commands,
    sim: SimView,
    mut hover: ResMut<HoverItem>,
    window: Single<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    tip: Single<(Entity, &mut Node, &mut BorderColor), With<Tooltip>>,
    mut shown: Local<Option<String>>,
) {
    let pinned = std::env::var("WOW_HOVER_ITEM").ok();
    let item = hover.0.take().or(pinned);
    let (entity, mut node, mut border) = tip.into_inner();
    let Some(item) = item else {
        node.display = Display::None;
        *shown = None;
        return;
    };
    let me = sim.player().and_then(|p| sim.unit(p));
    let equipped = me.map(|u| goods::slots_of(&u.equipped)).unwrap_or_default();
    let role = sim
        .player()
        .and_then(|p| sim.unit(p))
        .and_then(|u| sim.content().class_of_unit(&u.key))
        .map(|c| c.role);
    let key = format!("{item}|{}", equipped.join("|"));
    if shown.as_deref() != Some(key.as_str()) {
        *shown = Some(key);
        commands.entity(entity).despawn_related::<Children>();
        for (k, (text, color)) in goods::tooltip(sim.content(), &item, &equipped, role)
            .into_iter()
            .enumerate()
        {
            let size = if k == 0 { 15.0 } else { 12.0 };
            commands
                .entity(entity)
                .with_child((label(text, size), TextColor(color)));
        }
        *border = BorderColor::all(goods::name_color(sim.content(), &item).with_alpha(0.9));
    }
    node.display = Display::Flex;
    let at = window
        .cursor_position()
        .unwrap_or(Vec2::new(window.width() * 0.5, window.height() * 0.4))
        / scale.0.max(0.01);
    let (w, h) = (window.width() / scale.0, window.height() / scale.0);
    // Keep the tip on screen: flip left of the cursor near the right edge.
    let left = if at.x + 350.0 > w {
        at.x - 345.0
    } else {
        at.x + 18.0
    };
    node.left = px(left.max(4.0));
    // In the lower half the tip grows upward from the cursor, so long ones never run off screen.
    if at.y > h * 0.5 {
        node.top = Val::Auto;
        node.bottom = px((h - at.y + 8.0).max(4.0));
    } else {
        node.bottom = Val::Auto;
        node.top = px(at.y + 12.0);
    }
}

/// A square item tile: icon, rarity border, optional count badge; `Button` so it can be clicked.
pub fn item_tile(
    content: &combat_core::Content,
    icons: &mut crate::hud::IconCache,
    assets: &AssetServer,
    name: &str,
    size: f32,
    count: usize,
) -> impl Bundle {
    let icon = icons.get(assets, goods::icon_of(content, name));
    let border = goods::name_color(content, name);
    (
        Button,
        Node {
            width: px(size),
            height: px(size),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(3)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(border),
        children![
            (
                ImageNode::new(icon),
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                }
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    right: px(2),
                    bottom: px(0),
                    display: if count > 1 {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                label(format!("x{count}"), 11.0),
            ),
        ],
    )
}

/// Window chrome shared by the sheet and the shop.
pub fn window_node(left: Option<f32>, right: Option<f32>, width: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: left.map_or(Val::Auto, px),
            right: right.map_or(Val::Auto, px),
            top: px(84),
            width: px(width),
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(ACCENT.with_alpha(0.7)),
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(32),
    )
}
