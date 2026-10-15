//! `~/procurement`: the vendor window. Opens when you talk to a vendor NPC; lists the current
//! stock (with upgrade arrows, level requirements and prices), your bag (click to sell) and the
//! buyback shelf. Purchases of `CONFIRM_SP` or more need a second click; on touch every item
//! takes a first tap to inspect.

use super::goods::{self, SP_GOLD};
use super::sheet::grouped;
use super::{EconomyUi, HoverItem, Inspect, compare_badge, item_tile, tap_position, window_node};
use crate::combat::{PendingCommands, SimReady, SimView, Talked};
use crate::copy::{kudos, thousands};
use crate::hud::{DIM, IconCache, PANEL_EDGE, label, set_text};
use crate::player::{Player, PlayerBody};
use bevy::prelude::*;
use combat_core::{Command, EntityId, ticks_to_ms};

/// Purchases at or above this price ask "are you sure?" first.
const CONFIRM_SP: u32 = 500;
const CONFIRM_SECS: f32 = 4.0;
const ROW_ICON: f32 = 22.0;

#[derive(Component)]
struct ShopWindow;
#[derive(Component)]
struct ShopClose;
#[derive(Component)]
struct ShopTitle;
#[derive(Component)]
struct ShopStatus;
#[derive(Component)]
struct StockList;
#[derive(Component)]
struct SellGrid;
#[derive(Component)]
struct BuybackRow;
#[derive(Component)]
struct SellJunk;

#[derive(Component, Clone)]
enum ShopAction {
    Buy(String),
    Sell(String),
    Buyback(String),
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_window).add_systems(
            Update,
            (open_on_talk, debug_open, rebuild, clicks)
                .chain()
                .in_set(SimReady),
        );
    }
}

fn spawn_window(mut commands: Commands) {
    commands
        .spawn((ShopWindow, window_node(None, Some(16.0), 400.0)))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((ShopTitle, label("~/procurement", 14.0), TextColor(DIM)));
                h.spawn((ShopClose, Button, label("[x]", 14.0)));
            });
            c.spawn((ShopStatus, label("", 12.0), TextColor(SP_GOLD)));
            c.spawn((label("for sale (click to buy)", 12.0), TextColor(DIM)));
            c.spawn((
                StockList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1),
                    ..default()
                },
            ));
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("your bag (click to sell)", 12.0), TextColor(DIM)));
                h.spawn((
                    SellJunk,
                    Button,
                    Node {
                        padding: UiRect::axes(px(6), px(2)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    children![(label("sell all junk", 12.0), TextColor(SP_GOLD))],
                ));
            });
            c.spawn((
                SellGrid,
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(4),
                    row_gap: px(4),
                    min_height: px(36),
                    ..default()
                },
            ));
            c.spawn((label("buyback", 12.0), TextColor(DIM)));
            c.spawn((
                BuybackRow,
                Node {
                    column_gap: px(4),
                    min_height: px(36),
                    ..default()
                },
            ));
        });
}

fn open_on_talk(mut talked: MessageReader<Talked>, sim: SimView, mut ui: ResMut<EconomyUi>) {
    for Talked(id) in talked.read() {
        let is_vendor = sim
            .unit(*id)
            .is_some_and(|u| sim.content().economy.vendor_for(&u.key).is_some());
        if is_vendor {
            ui.shop = Some(*id);
            // The bag is the other half of a shop.
            ui.sheet = true;
        }
    }
}

/// `WOW_SHOP=1`: the nearest vendor's window opens two seconds in.
fn debug_open(
    time: Res<Time>,
    sim: SimView,
    player: Single<&PlayerBody, With<Player>>,
    mut ui: ResMut<EconomyUi>,
    mut done: Local<bool>,
) {
    if *done || std::env::var_os("WOW_SHOP").is_none() || time.elapsed_secs() < 2.0 {
        return;
    }
    let p = crate::collision::to_core(player.pos);
    let nearest = sim
        .units()
        .filter(|u| sim.content().economy.vendor_for(&u.key).is_some())
        .min_by(|a, b| combat_core::distance(a.pos, p).total_cmp(&combat_core::distance(b.pos, p)))
        .map(|u| u.id);
    if let Some(v) = nearest {
        *done = true;
        ui.shop = Some(v);
        ui.sheet = true;
    }
}

fn vendor_in_reach(sim: &dyn crate::combat::SimRead, vendor: EntityId, at: Vec3) -> bool {
    let reach = sim.content().economy.vendor_range_m + 1.0;
    sim.unit(vendor).is_some_and(|v| {
        v.alive && combat_core::distance(v.pos, crate::collision::to_core(at)) <= reach
    })
}

fn rebuild(
    mut commands: Commands,
    time: Res<Time>,
    mut ui: ResMut<EconomyUi>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&PlayerBody, With<Player>>,
    mut window: Single<&mut Node, With<ShopWindow>>,
    lists: Query<(
        Entity,
        Option<&StockList>,
        Option<&SellGrid>,
        Option<&BuybackRow>,
    )>,
    mut texts: Query<(&mut Text, Option<&ShopTitle>, Option<&ShopStatus>)>,
    close: Query<&Interaction, (With<ShopClose>, Changed<Interaction>)>,
    mut shown: Local<String>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        ui.shop = None;
    }
    if let Some(v) = ui.shop
        && !vendor_in_reach(&*sim, v, player.pos)
    {
        ui.shop = None;
    }
    if let Some((_, left)) = ui.confirm.as_mut() {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            ui.confirm = None;
        }
    }
    let (Some(vendor), Some(me)) = (ui.shop, sim.player().and_then(|p| sim.unit(p)).cloned())
    else {
        window.display = Display::None;
        shown.clear();
        return;
    };
    window.display = Display::Flex;
    let content = sim.content();
    let Some(def) = sim
        .unit(vendor)
        .and_then(|u| content.economy.vendor_for(&u.key))
    else {
        return;
    };
    let now_ms = ticks_to_ms(sim.now().0);
    let stock = content.economy.stock(def, now_ms);
    let confirm = ui.confirm.as_ref().map(|(n, _)| n.clone());
    let key = format!(
        "{stock:?}|{:?}|{:?}|{}|{confirm:?}|{:?}|{}",
        me.bag, me.buyback, me.sp, me.equipped, me.level
    );
    for (mut t, title, status) in &mut texts {
        if title.is_some() {
            set_text(&mut t, &def.title);
        } else if status.is_some() {
            let wallet = format!("Wallet: {}", kudos(me.sp));
            let approved = if def.rotate_count > 0 {
                format!(
                    "  |  Procurement approved {} extra items this week",
                    def.rotate_count
                )
            } else {
                String::new()
            };
            let msg = match &confirm {
                Some(n) => format!(
                    "{wallet}. Click [{n}] again within a few seconds to approve the purchase order."
                ),
                None => format!("{wallet}{approved}\n\"{}\"", def.greeting),
            };
            set_text(&mut t, &msg);
        }
    }
    if *shown == key {
        return;
    }
    *shown = key;
    let equipped = goods::slots_of(&me.equipped);
    let role = content.class_of_unit(&me.key).map(|c| c.role);
    for (e, stock_list, sell, buyback) in &lists {
        if stock_list.is_some() {
            commands.entity(e).despawn_related::<Children>();
            for name in &stock {
                let price = content.economy.goods(name).map_or(0, |g| g.price());
                let affordable = me.sp >= price;
                let need = content
                    .economy
                    .gear(name)
                    .map_or(1, combat_core::GearDef::required_level);
                let locked = me.level < need;
                let ord = goods::compare(content, name, &equipped, role);
                let pending = confirm.as_deref() == Some(name.as_str());
                let row = commands
                    .spawn((
                        ShopAction::Buy(name.clone()),
                        Button,
                        Node {
                            column_gap: px(6),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(px(2)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(if pending {
                            SP_GOLD
                        } else {
                            PANEL_EDGE.with_alpha(0.0)
                        }),
                    ))
                    .with_children(|r| {
                        let mut tile =
                            r.spawn(item_tile(content, &mut icons, &assets, name, ROW_ICON, 1));
                        if let Some(b) = compare_badge(ord) {
                            tile.with_child(b);
                        }
                        let name_color = if locked {
                            goods::JUNK_GREY.with_alpha(0.7)
                        } else {
                            goods::name_color(content, name)
                        };
                        r.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            ..default()
                        })
                        .with_children(|t| {
                            t.spawn((label(name.clone(), 11.0), TextColor(name_color)));
                            if locked {
                                t.spawn((
                                    label(format!("Requires Level {need}"), 10.0),
                                    TextColor(goods::DOWNGRADE_RED.with_alpha(0.8)),
                                ));
                            }
                        });
                        let price_color = if locked {
                            goods::JUNK_GREY
                        } else if affordable {
                            SP_GOLD
                        } else {
                            goods::DOWNGRADE_RED
                        };
                        r.spawn((
                            label(format!("{} K", thousands(price)), 12.0),
                            TextColor(price_color),
                        ));
                    })
                    .id();
                if locked {
                    commands
                        .entity(row)
                        .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)));
                }
                commands.entity(e).add_child(row);
            }
        } else if sell.is_some() {
            commands.entity(e).despawn_related::<Children>();
            for (name, count) in grouped(content, &me.bag) {
                let mut tile = commands.spawn((
                    item_tile(content, &mut icons, &assets, &name, 36.0, count),
                    ShopAction::Sell(name.clone()),
                ));
                if let Some(b) = compare_badge(goods::compare(content, &name, &equipped, role)) {
                    tile.with_child(b);
                }
                let tile = tile.id();
                commands.entity(e).add_child(tile);
            }
        } else if buyback.is_some() {
            commands.entity(e).despawn_related::<Children>();
            for name in me.buyback.iter().rev() {
                let tile = commands
                    .spawn((
                        item_tile(content, &mut icons, &assets, name, 36.0, 1),
                        ShopAction::Buyback(name.clone()),
                    ))
                    .id();
                commands.entity(e).add_child(tile);
            }
        }
    }
}

fn clicks(
    mut ui: ResMut<EconomyUi>,
    sim: SimView,
    touch: Res<crate::touch::TouchMode>,
    touches: Res<bevy::input::touch::Touches>,
    mut hover: ResMut<HoverItem>,
    mut inspect: ResMut<Inspect>,
    mut pending: ResMut<PendingCommands>,
    mut watch: ResMut<super::discard::SweepWatch>,
    sell_junk: Query<&Interaction, (With<SellJunk>, Changed<Interaction>)>,
    pressed: Query<(&Interaction, &ShopAction), Changed<Interaction>>,
    hovered: Query<(&Interaction, &ShopAction)>,
) {
    let Some(vendor) = ui.shop else { return };
    for (i, a) in &hovered {
        if *i != Interaction::None {
            let (ShopAction::Buy(n) | ShopAction::Sell(n) | ShopAction::Buyback(n)) = a;
            hover.set(n, matches!(a, ShopAction::Buy(_)));
        }
    }
    if sell_junk.iter().any(|i| *i == Interaction::Pressed)
        && let Some(me) = sim.player().and_then(|p| sim.unit(p))
        && me.bag.iter().any(|n| super::discard::is_junk(&*sim, n))
    {
        let sweep = super::discard::Sweep::Sell {
            bag_len: me.bag.len(),
            sp: me.sp,
        };
        watch.0 = Some((sweep, 3.0));
        pending.0.push(Command::SellJunk { vendor });
    }
    for (i, a) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        let (ShopAction::Buy(n) | ShopAction::Sell(n) | ShopAction::Buyback(n)) = a;
        let buying = matches!(a, ShopAction::Buy(_));
        if !inspect.gate(touch.active, n, buying, tap_position(&touches)) {
            continue;
        }
        match a {
            ShopAction::Buy(item) => {
                let price = sim.content().economy.goods(item).map_or(0, |g| g.price());
                let confirmed = ui.confirm.as_ref().is_some_and(|(n, _)| n == item);
                if price >= CONFIRM_SP && !confirmed {
                    ui.confirm = Some((item.clone(), CONFIRM_SECS));
                    continue;
                }
                ui.confirm = None;
                pending.0.push(Command::Buy {
                    vendor,
                    item: item.clone(),
                });
            }
            ShopAction::Sell(item) => pending.0.push(Command::Sell {
                vendor,
                item: item.clone(),
            }),
            ShopAction::Buyback(item) => pending.0.push(Command::Buyback {
                vendor,
                item: item.clone(),
            }),
        }
    }
}
