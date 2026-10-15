//! Client side of the economy: the `~/whoami` character sheet (equipment, stats, bag), the
//! `~/procurement` vendor window, the gadget bar for on-use gear, item tooltips, gear worn
//! visibly on every player model, loot beams, and the offline save.
//!
//! Debug and screenshot switches:
//! - `WOW_GEAR=<item>|<item>..`: equip these pieces on the local player at start (offline).
//! - `WOW_SP=<n>`: start with that many Kudos (`WOW_SP_GAIN=<n>@<secs>` grants more later, for the
//!   wallet's "+N Kudos" float); `WOW_BAG=<item>|..` puts goods in the bag.
//! - `WOW_SHEET=1`: open the character sheet; `WOW_SHOP=1`: open the nearest vendor two seconds in.
//! - `WOW_HOVER_ITEM=<item>`: pin that item's tooltip (screenshots of comparisons);
//!   `WOW_HOVER_BUY=1` shows it as on a vendor shelf (price, level requirement).
//! - `WOW_LINEUP_GEAR=1`: lineup skins (`WOW_LINEUP=4`) wear a sample outfit per class.
//! - `WOW_NO_SAVE=1`: neither load nor write the offline save.
//! - `WOW_LOOT=<secs>`: loot the nearest corpse within reach at that time (loot window shots).

// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod goods;
mod outfit;
pub mod save;
mod sheet;
mod shop;
mod wallet;

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
pub struct HoverItem {
    pub name: Option<String>,
    /// Hovered on a vendor's shelf (the tooltip shows price and level requirement).
    pub buying: bool,
}

impl HoverItem {
    pub fn set(&mut self, name: &str, buying: bool) {
        self.name = Some(name.to_string());
        self.buying = buying;
    }
}

/// Touch has no hover: the first tap on an item pins its tooltip (tap-to-inspect), a second tap
/// on the same item acts. Cleared when the item windows close.
#[derive(Resource, Default)]
pub struct Inspect {
    pub name: Option<String>,
    pub buying: bool,
    /// Where the tap landed, in window logical px.
    pub at: Vec2,
    /// Pinned from the loot window, which closes its own pins.
    pub loot: bool,
}

impl Inspect {
    /// True when a press on `name` should act now; on touch the first tap only inspects.
    pub fn gate(&mut self, touch: bool, name: &str, buying: bool, at: Option<Vec2>) -> bool {
        if !touch || self.name.as_deref() == Some(name) {
            self.name = None;
            return true;
        }
        self.name = Some(name.to_string());
        self.buying = buying;
        self.at = at.unwrap_or(self.at);
        self.loot = false;
        false
    }
}

/// Where the latest touch started (window logical px), for pinning a tooltip.
pub fn tap_position(touches: &bevy::input::touch::Touches) -> Option<Vec2> {
    touches
        .iter_just_pressed()
        .chain(touches.iter())
        .map(bevy::input::touch::Touch::position)
        .next()
}

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EconomyUi>()
            .init_resource::<HoverItem>()
            .init_resource::<Inspect>()
            .add_systems(PreUpdate, close_on_escape.after(InputSystems))
            .add_systems(Startup, spawn_tooltip)
            .add_systems(PostStartup, debug_start.in_set(SimReady))
            .add_systems(
                Update,
                (update_tooltip, debug_loot, debug_gain).in_set(SimReady),
            )
            .add_systems(Update, pin_ui_camera)
            .add_plugins((
                wallet::WalletPlugin,
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

fn debug_gain(time: Res<Time>, mut sim: ResMut<CombatSim>, mut done: Local<bool>) {
    let Some((n, at)) = std::env::var("WOW_SP_GAIN").ok().and_then(|v| {
        let (n, at) = v.split_once('@')?;
        Some((n.parse::<u32>().ok()?, at.parse::<f32>().ok()?))
    }) else {
        return;
    };
    if *done || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    if let Some(p) = sim.0.player() {
        sim.0.grant_sp(p, n);
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

/// 0: the hovered item, 1: the piece worn in its slot.
#[derive(Component)]
struct TipPanel(usize);

const TIP_W: f32 = 300.0;
const TIP_EDGE: f32 = 4.0;

fn spawn_tooltip(mut commands: Commands) {
    let panel = |k: usize| {
        (
            TipPanel(k),
            Node {
                width: px(TIP_W),
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            // Opaque: the tip often sits over the shop list, whose text must not bleed through.
            BackgroundColor(Color::srgb(0.04, 0.045, 0.07)),
            drop_shadow(),
            GlobalZIndex(60),
        )
    };
    commands.spawn((
        Tooltip,
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::FlexStart,
            column_gap: px(6),
            row_gap: px(6),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(60),
        children![panel(0), panel(1)],
    ));
}

fn update_tooltip(
    mut commands: Commands,
    sim: SimView,
    ui: Res<EconomyUi>,
    touch: Res<crate::touch::TouchMode>,
    mut hover: ResMut<HoverItem>,
    mut inspect: ResMut<Inspect>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Query<&Camera, With<crate::camera::OrbitCamera>>,
    scale: Res<UiScale>,
    tip: Single<(&mut Node, &ComputedNode), With<Tooltip>>,
    mut panels: Query<(Entity, &TipPanel, &mut Node, &mut BorderColor), Without<Tooltip>>,
    mut shown: Local<Option<String>>,
) {
    if !ui.sheet && ui.shop.is_none() && !inspect.loot {
        inspect.name = None;
    }
    let pinned = std::env::var("WOW_HOVER_ITEM")
        .ok()
        .map(|n| (n, std::env::var_os("WOW_HOVER_BUY").is_some()));
    let tapped = inspect.name.clone().map(|n| (n, inspect.buying));
    let hovered = hover.name.take().map(|n| (n, hover.buying));
    let (mut node, computed) = tip.into_inner();
    let Some((item, buying)) = hovered.or(tapped).or(pinned) else {
        node.display = Display::None;
        *shown = None;
        return;
    };
    let me = sim.player().and_then(|p| sim.unit(p));
    let equipped = me.map(|u| goods::slots_of(&u.equipped)).unwrap_or_default();
    let role = me
        .and_then(|u| sim.content().class_of_unit(&u.key))
        .map(|c| c.role);
    let (level, kudos) = me.map_or((1, 0), |u| (u.level, u.sp));
    // The play camera's viewport is the HUD's screen (it differs from the window in screenshots).
    let view = camera
        .iter()
        .next()
        .and_then(Camera::logical_viewport_size)
        .unwrap_or(Vec2::new(window.width(), window.height()));
    let (w, h) = (view.x / scale.0.max(0.01), view.y / scale.0.max(0.01));
    let panel_w = TIP_W.min(w - 2.0 * TIP_EDGE);
    let key = format!(
        "{item}|{buying}|{level}|{kudos}|{panel_w}|{}",
        equipped.join("|")
    );
    if shown.as_deref() != Some(key.as_str()) {
        *shown = Some(key);
        let viewer = goods::Viewer {
            equipped: &equipped,
            role,
            level,
            kudos,
            buying,
        };
        let tip = goods::tooltip(sim.content(), &item, &viewer);
        for (entity, panel, mut pnode, mut border) in &mut panels {
            commands.entity(entity).despawn_related::<Children>();
            pnode.width = px(panel_w);
            let lines = if panel.0 == 0 {
                Some(&tip.main)
            } else {
                tip.worn.as_ref()
            };
            let Some(lines) = lines else {
                pnode.display = Display::None;
                continue;
            };
            pnode.display = Display::Flex;
            for (k, (text, color)) in lines.iter().enumerate() {
                // The worn panel's first line is its "Currently equipped" caption.
                let title = (panel.0 == 0 && k == 0) || (panel.0 == 1 && k == 1);
                let size = if title { 15.0 } else { 12.0 };
                commands
                    .entity(entity)
                    .with_child((label(text.clone(), size), TextColor(*color)));
            }
            let edge = lines
                .get(panel.0)
                .map_or(PANEL_EDGE, |(_, c)| c.with_alpha(0.9));
            *border = BorderColor::all(edge);
        }
    }
    node.display = Display::Flex;
    node.max_width = px(w - 2.0 * TIP_EDGE);
    let cursor = window.cursor_position().map(|c| c / scale.0.max(0.01));
    let at = if touch.active && inspect.name.as_deref() == Some(item.as_str()) {
        inspect.at / scale.0.max(0.01)
    } else {
        cursor.unwrap_or(Vec2::new(w * 0.5, h * 0.4))
    };
    // Last frame's laid-out size (UI px); the first frame of a new tip may be a frame late.
    let size = computed.size() * computed.inverse_scale_factor();
    let (left, top) = clamp_tip(at, size, Vec2::new(w, h));
    node.left = px(left);
    node.top = px(top);
    node.bottom = Val::Auto;
}

/// Top-left corner for a tip of `size` near the pointer `at`: right of and below it, flipped to
/// the other side where it would leave the `screen`, and always fully inside it when it fits.
fn clamp_tip(at: Vec2, size: Vec2, screen: Vec2) -> (f32, f32) {
    let mut left = at.x + 18.0;
    if left + size.x > screen.x - TIP_EDGE {
        left = at.x - size.x - 12.0;
    }
    let mut top = at.y + 14.0;
    if top + size.y > screen.y - TIP_EDGE {
        top = at.y - size.y - 8.0;
    }
    let max = (screen - size - Vec2::splat(TIP_EDGE)).max(Vec2::splat(TIP_EDGE));
    (left.clamp(TIP_EDGE, max.x), top.clamp(TIP_EDGE, max.y))
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

/// Corner arrow on an item icon: green up for an upgrade, red down for a downgrade, nothing for
/// sidegrades and non-gear.
pub fn compare_badge(ord: Option<std::cmp::Ordering>) -> Option<impl Bundle> {
    let (glyph, color) = match ord? {
        std::cmp::Ordering::Greater => ("\u{25B2}", goods::UPGRADE_GREEN),
        std::cmp::Ordering::Less => ("\u{25BC}", goods::DOWNGRADE_RED),
        std::cmp::Ordering::Equal => return None,
    };
    Some((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(12),
            height: px(12),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::bottom_right(px(4)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        children![(
            Text::new(glyph),
            TextFont::from_font_size(10.0),
            TextColor(color)
        )],
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltips_stay_on_screen_at_any_size() {
        let screen = Vec2::new(800.0, 450.0);
        for at in [
            Vec2::ZERO,
            Vec2::new(799.0, 449.0),
            Vec2::new(400.0, 10.0),
            Vec2::new(10.0, 440.0),
        ] {
            let size = Vec2::new(606.0, 300.0);
            let (l, t) = clamp_tip(at, size, screen);
            assert!(
                l >= TIP_EDGE && l + size.x <= screen.x - TIP_EDGE + 0.01,
                "{at} {l}"
            );
            assert!(
                t >= TIP_EDGE && t + size.y <= screen.y - TIP_EDGE + 0.01,
                "{at} {t}"
            );
        }
        // Too big to fit: pinned to the top-left corner instead of off screen.
        assert_eq!(
            clamp_tip(Vec2::new(300.0, 300.0), Vec2::new(900.0, 900.0), screen),
            (TIP_EDGE, TIP_EDGE)
        );
    }

    #[test]
    fn touch_needs_a_second_tap_to_act() {
        let mut i = Inspect::default();
        assert!(i.gate(false, "Mug", false, None));
        assert!(!i.gate(true, "Mug", false, Some(Vec2::ONE)));
        assert_eq!(i.name.as_deref(), Some("Mug"));
        assert!(!i.gate(true, "Cap", true, None));
        assert!(i.gate(true, "Cap", true, None));
        assert!(i.name.is_none());
    }
}
