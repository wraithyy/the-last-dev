//! `~/whoami` (I): paper doll around a live 3D preview of your character, the stat summary in
//! dev terms, the bag (click to equip or use), and the gadget bar for on-use gear.

use super::goods;
use super::{EconomyUi, HoverItem, item_tile, window_node};
use crate::combat::{PendingCommands, SimRead, SimReady, SimView};
use crate::hud::{ACCENT, DIM, IconCache, PANEL_EDGE, drop_shadow, label, panel_bg, set_text};
use crate::input::Paused;
use crate::keybinds::{Action, Keybinds};
use crate::player::{Player, PlayerBody};
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use combat_core::{Command, Goods, Perk, SLOTS, Slot};

const TILE: f32 = 48.0;
const BAG_TILE: f32 = 40.0;
const PREVIEW: (u32, u32) = (224, 288);

#[derive(Component)]
struct SheetWindow;
#[derive(Component)]
struct SheetClose;
#[derive(Component)]
struct SlotBox(usize);
#[derive(Component)]
struct BagGrid;
#[derive(Component)]
struct StatsText;
#[derive(Component)]
struct HeaderText;
#[derive(Component)]
struct PerkText;
#[derive(Component)]
struct PreviewCam;
#[derive(Component)]
struct PreviewImage;
#[derive(Resource)]
struct PreviewTarget(Handle<Image>);

/// What a click on a tile does.
#[derive(Component, Clone)]
pub enum TileAction {
    Unequip(u8),
    Equip(String),
    Use(String),
}

#[derive(Component)]
struct TileItem(String);

#[derive(Component)]
struct GadgetBar;
#[derive(Component)]
struct GadgetButton(usize);
#[derive(Component)]
struct GadgetCooldown(usize);

pub struct SheetPlugin;

impl Plugin for SheetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_window, spawn_gadgets))
            .add_systems(
                Update,
                (toggle, rebuild, tiles, follow_preview, gadgets)
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_window(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // No separate view format: the target is used as is.
    let image = images.add(Image::new_target_texture(
        PREVIEW.0,
        PREVIEW.1,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.insert_resource(PreviewTarget(image.clone()));
    let slot_box = |c: &mut ChildSpawnerCommands, k: usize| {
        c.spawn((
            SlotBox(k),
            Node {
                width: px(TILE + 4.0),
                height: px(TILE + 4.0),
                ..default()
            },
        ));
    };
    commands
        .spawn((SheetWindow, window_node(Some(16.0), None, 600.0)))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((HeaderText, label("~/whoami", 14.0), TextColor(DIM)));
                // Fixed width: a long header line used to squeeze this button onto two lines.
                h.spawn((
                    crate::social::PetsButton,
                    Button,
                    Node {
                        flex_shrink: 0.0,
                        margin: UiRect::horizontal(px(8)),
                        ..default()
                    },
                    children![label("[~/pets Z]", 14.0)],
                ));
                h.spawn((SheetClose, Button, label("[x]", 14.0)));
            });
            c.spawn(Node {
                column_gap: px(8),
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: UiRect::top(px(20)),
                    ..default()
                })
                .with_children(|col| {
                    for k in [0, 1, 2] {
                        slot_box(col, k);
                    }
                });
                row.spawn((
                    PreviewImage,
                    ImageNode::new(image.clone()),
                    Node {
                        width: px(PREVIEW.0 as f32 * 0.8),
                        height: px(PREVIEW.1 as f32 * 0.8),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                ));
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: UiRect::top(px(20)),
                    ..default()
                })
                .with_children(|col| {
                    for k in [3, 4] {
                        slot_box(col, k);
                    }
                });
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((StatsText, label("", 12.0), TextColor(Color::WHITE)));
                    col.spawn((PerkText, label("", 11.0), TextColor(goods::PERK_GOLD)));
                });
            });
            super::discard::bag_header(c, "bag (click: equip / use, shift+right-click: delete)");
            c.spawn((
                BagGrid,
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(4),
                    row_gap: px(4),
                    min_height: px(BAG_TILE),
                    ..default()
                },
            ));
        });
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    mut ui: ResMut<EconomyUi>,
    close: Query<&Interaction, (With<SheetClose>, Changed<Interaction>)>,
    mut window: Single<&mut Node, With<SheetWindow>>,
    mut commands: Commands,
    cams: Query<Entity, With<PreviewCam>>,
    target: Res<PreviewTarget>,
) {
    if !paused.0 && binds.just(&keys, Action::CharacterSheet) {
        ui.sheet = !ui.sheet;
    }
    if close.iter().any(|i| *i == Interaction::Pressed) {
        ui.sheet = false;
    }
    window.display = if ui.sheet {
        Display::Flex
    } else {
        Display::None
    };
    // The preview camera exists only while the sheet is open (it renders the scene a second time).
    match (ui.sheet, cams.iter().next()) {
        (true, None) => {
            commands.spawn((
                PreviewCam,
                Camera3d::default(),
                Camera {
                    order: -1,
                    clear_color: Color::srgb(0.06, 0.07, 0.1).into(),
                    ..default()
                },
                RenderTarget::Image(target.0.clone().into()),
                Transform::default(),
            ));
        }
        (false, Some(e)) => commands.entity(e).despawn(),
        _ => {}
    }
}

/// Bag entries in first-seen order; consumables and junk stack visually, gear stays one tile
/// per piece.
pub fn grouped(content: &combat_core::Content, bag: &[String]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for n in bag {
        let stacks = content.economy.gear(n).is_none();
        match out.iter_mut().find(|(m, _)| m == n) {
            Some((_, c)) if stacks => *c += 1,
            _ => out.push((n.clone(), 1)),
        }
    }
    out
}

fn rebuild(
    mut commands: Commands,
    ui: Res<EconomyUi>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    slots: Query<(Entity, &SlotBox)>,
    grid: Single<Entity, With<BagGrid>>,
    mut texts: Query<(
        &mut Text,
        Option<&StatsText>,
        Option<&HeaderText>,
        Option<&PerkText>,
    )>,
    mut shown: Local<String>,
) {
    if !ui.sheet {
        shown.clear();
        return;
    }
    let Some(me) = sim.player().and_then(|p| sim.unit(p)).cloned() else {
        return;
    };
    let content = sim.content();
    let equipped = goods::slots_of(&me.equipped);
    let totals = sim.item_totals();
    let key = format!(
        "{:?}|{:?}|{}|{}|{}|{}",
        equipped, me.bag, me.sp, me.level, me.max_hp, me.max_mana
    );
    if *shown == key {
        return;
    }
    *shown = key;
    for (e, slot) in &slots {
        commands.entity(e).despawn_related::<Children>();
        let name = &equipped[slot.0];
        let label_text = Slot::ALL[slot.0].label();
        if name.is_empty() {
            commands.entity(e).with_child((
                Button,
                Node {
                    width: px(TILE),
                    height: px(TILE),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(crate::hud::BAR_BG),
                children![(label(label_text, 9.0), TextColor(DIM))],
            ));
        } else {
            commands.entity(e).with_child((
                item_tile(content, &mut icons, &assets, name, TILE, 1),
                TileItem(name.clone()),
                TileAction::Unequip(slot.0 as u8),
            ));
        }
    }
    commands.entity(*grid).despawn_related::<Children>();
    let role = content.class_of_unit(&me.key).map(|c| c.role);
    for (name, count) in grouped(content, &me.bag) {
        let action = match content.economy.goods(&name) {
            Some(Goods::Gear(_)) => Some(TileAction::Equip(name.clone())),
            Some(Goods::Consumable(_)) => Some(TileAction::Use(name.clone())),
            _ => None,
        };
        let ord = goods::compare(content, &name, &equipped, role);
        let mut tile = commands.spawn((
            item_tile(content, &mut icons, &assets, &name, BAG_TILE, count),
            TileItem(name.clone()),
        ));
        if let Some(a) = action {
            tile.insert(a);
        }
        if let Some(b) = super::compare_badge(ord) {
            tile.with_child(b);
        }
        let id = tile.id();
        commands.entity(*grid).add_child(id);
    }
    let empty = content.economy.bag_size.saturating_sub(me.bag.len());
    for _ in 0..empty {
        commands.entity(*grid).with_child((
            Node {
                width: px(BAG_TILE),
                height: px(BAG_TILE),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE.with_alpha(0.4)),
            BackgroundColor(crate::hud::BAR_BG.with_alpha(0.6)),
        ));
    }
    let class = content.class_of_unit(&me.key);
    let header = format!(
        "~/whoami  |  {} {} lv {}  |  gear score {}",
        class.map_or("", |c| c.name.as_str()),
        class.map_or("", |c| c.role.tag()),
        me.level,
        goods::gear_score(content, &equipped)
    );
    let mut stats = vec![
        format!("Wallet: {}", crate::copy::kudos(me.sp)),
        format!("HP {} / mana {}", me.max_hp, me.max_mana),
        String::new(),
    ];
    stats.extend(goods::sheet_lines(&totals));
    let (_, perks) = content.economy.equipment(&equipped);
    let perk_text = perks
        .iter()
        .map(|p| {
            let text = content
                .economy
                .gear(&p.source)
                .map(|g| g.perk_text.clone())
                .or_else(|| {
                    content
                        .economy
                        .sets
                        .iter()
                        .find(|s| s.name == p.source)
                        .map(|s| s.four_text.clone())
                })
                .unwrap_or_default();
            format!("{}: {text}", p.source)
        })
        .collect::<Vec<_>>()
        .join("\n");
    for (mut t, s, h, p) in &mut texts {
        if s.is_some() {
            set_text(&mut t, &stats.join("\n"));
        } else if h.is_some() {
            set_text(&mut t, &header);
        } else if p.is_some() {
            set_text(&mut t, &perk_text);
        }
    }
}

/// A bag tile pressed with the mouse: acts on release unless it was dragged out of the window.
struct Held {
    item: String,
    action: Option<TileAction>,
}

fn tiles(
    ui: Res<EconomyUi>,
    sim: SimView,
    touch: Res<crate::touch::TouchMode>,
    touches: Res<bevy::input::touch::Touches>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    sheet: Single<(&ComputedNode, &UiGlobalTransform), With<SheetWindow>>,
    mut hover: ResMut<HoverItem>,
    mut inspect: ResMut<super::Inspect>,
    mut pending: ResMut<PendingCommands>,
    mut discard: ResMut<super::discard::DiscardUi>,
    mut held: Local<Option<Held>>,
    tiles: Query<(&Interaction, &TileItem, Option<&TileAction>), Changed<Interaction>>,
    hovered: Query<(&Interaction, &TileItem, Option<&TileAction>)>,
) {
    if !ui.sheet || sim.player().is_none() {
        *held = None;
        return;
    }
    let worn = |a: Option<&TileAction>| matches!(a, Some(TileAction::Unequip(_)));
    for (i, item, _) in &hovered {
        if *i != Interaction::None {
            hover.set(&item.0, false);
        }
    }
    // Equipped pieces never reach the discard path; the sim refuses protected bag items too.
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if shift && mouse.just_pressed(MouseButton::Right) {
        let target = hovered
            .iter()
            .find(|(i, _, a)| **i != Interaction::None && !worn(*a));
        if let Some((_, item, _)) = target {
            super::discard::request(&mut discard, &mut pending, &*sim, &item.0);
        }
    }
    if mouse.just_released(MouseButton::Left)
        && let Some(h) = held.take()
    {
        let (node, at) = *sheet;
        let inside = window
            .physical_cursor_position()
            .is_none_or(|c| node.contains_point(*at, c));
        if !inside {
            super::discard::request(&mut discard, &mut pending, &*sim, &h.item);
        } else if hovered
            .iter()
            .any(|(i, t, _)| *i != Interaction::None && t.0 == h.item)
            && inspect.gate(touch.active, &h.item, false, super::tap_position(&touches))
        {
            act(h.action.as_ref(), &mut pending);
        }
    }
    for (i, item, action) in &tiles {
        if *i != Interaction::Pressed {
            continue;
        }
        // While the shop is open, bag clicks sell instead (the shop window handles those).
        if ui.shop.is_some() && !worn(action) {
            continue;
        }
        // Mouse presses on bag tiles wait for the release so a drag out can delete instead.
        if !touch.active && !worn(action) {
            *held = Some(Held {
                item: item.0.clone(),
                action: action.cloned(),
            });
            continue;
        }
        if !inspect.gate(touch.active, &item.0, false, super::tap_position(&touches)) {
            continue;
        }
        act(action, &mut pending);
    }
}

fn act(action: Option<&TileAction>, pending: &mut PendingCommands) {
    match action {
        Some(TileAction::Unequip(slot)) => pending.0.push(Command::Unequip { slot: *slot }),
        Some(TileAction::Equip(item)) => pending.0.push(Command::Equip { item: item.clone() }),
        Some(TileAction::Use(item)) => pending.0.push(Command::UseItem { item: item.clone() }),
        None => {}
    }
}

fn follow_preview(
    ui: Res<EconomyUi>,
    player: Single<&PlayerBody, With<Player>>,
    mut cams: Query<&mut Transform, With<PreviewCam>>,
) {
    let Some(mut cam) = cams.iter_mut().next().filter(|_| ui.sheet) else {
        return;
    };
    let rot = Quat::from_rotation_y(player.yaw);
    let front = rot * Vec3::NEG_Z;
    let side = rot * Vec3::X;
    let focus = player.pos + Vec3::Y * 0.95;
    // A three-quarter view from the front: the hoodie, the mug hand and the keyboard on the back.
    let eye = focus + front * 3.0 + side * 1.1 + Vec3::Y * 0.35;
    *cam = Transform::from_translation(eye).looking_at(focus, Vec3::Y);
}

// ---- gadget bar: equipped on-use items on U / O / P ----

fn spawn_gadgets(mut commands: Commands) {
    let (left, bottom) = crate::hud::actionbar::gadget_anchor();
    commands
        .spawn((
            GadgetBar,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                margin: UiRect::left(px(left)),
                bottom: px(bottom),
                padding: UiRect::all(px(4)),
                column_gap: px(4),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.5)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(20),
        ))
        .with_children(|c| {
            for k in 0..3 {
                c.spawn((
                    GadgetButton(k),
                    Button,
                    Node {
                        width: px(44),
                        height: px(44),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(3)),
                        overflow: Overflow::clip(),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    ImageNode::default(),
                ))
                .with_children(|b| {
                    b.spawn((
                        GadgetCooldown(k),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            right: px(0),
                            bottom: px(0),
                            height: percent(0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                    ));
                    b.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(2),
                            top: px(0),
                            ..default()
                        },
                        label(["U", "O", "P"][k], 11.0),
                    ));
                });
            }
        });
}

/// Equipped pieces with an on-use perk, in slot order (at most three).
pub fn gadgets_of(sim: &dyn SimRead) -> Vec<String> {
    let Some(me) = sim.player().and_then(|p| sim.unit(p)) else {
        return Vec::new();
    };
    me.equipped
        .iter()
        .take(SLOTS)
        .filter(|n| {
            sim.content()
                .economy
                .gear(n)
                .is_some_and(|g| matches!(g.perk, Some(Perk::OnUse { .. })))
        })
        .take(3)
        .cloned()
        .collect()
}

fn gadgets(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<Paused>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut pending: ResMut<PendingCommands>,
    mut hover: ResMut<HoverItem>,
    mut bar: Single<
        &mut Node,
        (
            With<GadgetBar>,
            Without<GadgetButton>,
            Without<GadgetCooldown>,
        ),
    >,
    mut buttons: Query<
        (
            &GadgetButton,
            &Interaction,
            &mut Node,
            &mut ImageNode,
            &mut BorderColor,
        ),
        Without<GadgetCooldown>,
    >,
    mut shades: Query<(&GadgetCooldown, &mut Node), Without<GadgetButton>>,
    touch: Res<crate::touch::TouchMode>,
    ui: Res<crate::touch::TouchUi>,
) {
    use crate::touch::{BTN, GADGET_TRAY_BOTTOM, GADGET_TRAY_RIGHT};
    let list = gadgets_of(&*sim);
    // Touch folds the gadgets into the "more" tray next to sprint and ride.
    let shown = !list.is_empty() && (!touch.active || ui.more);
    bar.display = if shown { Display::Flex } else { Display::None };
    let (left, right, margin, bottom) = if touch.active {
        (
            Val::Auto,
            px(GADGET_TRAY_RIGHT),
            Val::Px(0.0),
            px(GADGET_TRAY_BOTTOM),
        )
    } else {
        let (l, b) = crate::hud::actionbar::gadget_anchor();
        (percent(50), Val::Auto, Val::Px(l), px(b))
    };
    let margin = UiRect::left(margin);
    if bar.left != left || bar.right != right || bar.bottom != bottom || bar.margin != margin {
        bar.left = left;
        bar.right = right;
        bar.bottom = bottom;
        bar.top = Val::Auto;
        bar.margin = margin;
    }
    let side = px(if touch.active { BTN } else { 44.0 });
    let Some(me) = sim.player() else { return };
    for (b, interaction, mut node, mut image, mut border) in &mut buttons {
        let Some(item) = list.get(b.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        if node.width != side {
            node.width = side;
            node.height = side;
        }
        let icon = icons.get(&assets, goods::icon_of(sim.content(), item));
        if image.image != icon {
            image.image = icon;
        }
        *border = BorderColor::all(goods::name_color(sim.content(), item));
        if *interaction == Interaction::Hovered {
            hover.set(item, false);
        }
        let pressed = *interaction == Interaction::Pressed
            || (!paused.0 && binds.just(&keys, Action::Gadget(b.0 as u8)));
        if pressed {
            pending.0.push(Command::UseItem { item: item.clone() });
        }
    }
    for (g, mut node) in &mut shades {
        let (left, total) = list
            .get(g.0)
            .map_or((0, 0), |item| sim.item_cooldown(me, item));
        let pct = if total == 0 {
            0.0
        } else {
            left as f32 * 100.0 / total as f32
        };
        node.height = percent(pct);
    }
}
