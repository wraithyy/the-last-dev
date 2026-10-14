//! `~/trophies`: the inventory window (B). Shows every trophy the player looted as an icon tile with its
//! stack count; hovering a tile fills the detail pane, and the footer sums the permanent bonuses.
//! The second tab lists the secret achievements (`combat_core::world`): locked ones show a
//! hint, unlocked ones their title, and a click wears that title under your name.

use super::{ACCENT, DIM, IconCache, PANEL_EDGE, drop_shadow, label, panel_bg, set_text};
use crate::combat::{PendingCommands, SimRead, SimReady, SimView};
use crate::copy::{self, BONUS_GREEN};
use crate::input::{Paused, TrophiesOpen};
use bevy::prelude::*;
use combat_core::Command;
use combat_core::ITEM_STACK_CAP;

const TILE: f32 = 44.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);

#[derive(Component)]
struct TrophyWindow;
#[derive(Component)]
struct TrophyGrid;
#[derive(Component)]
struct TrophyClose;
#[derive(Component)]
struct TrophyTile(String);
#[derive(Component)]
struct TrophyTotals;
#[derive(Component)]
enum Detail {
    Name,
    Bonus,
    Desc,
}

const HINT: &str = "Hover a trophy.";
const ACH_HINT: &str = "Hover an achievement. Click an unlocked one to wear its title.";
const LOCKED: Color = Color::srgb(0.45, 0.45, 0.5);

/// Tab header: false = trophies, true = achievements.
#[derive(Component)]
struct TabButton(bool);
#[derive(Component)]
struct TrophyPane;
#[derive(Component)]
struct AchPane;
#[derive(Component)]
struct AchList;
#[derive(Component)]
struct AchRow(String);
#[derive(Component)]
struct AchTitle;
#[derive(Component)]
enum AchDetail {
    Name,
    Text,
}
#[derive(Resource, Default)]
struct AchTab(bool);

pub struct TrophiesPlugin;

impl Plugin for TrophiesPlugin {
    fn build(&self, app: &mut App) {
        // QA: `WOW_ACH_TAB=1` opens the window on the achievements tab.
        app.insert_resource(AchTab(std::env::var_os("WOW_ACH_TAB").is_some()))
            .add_systems(Startup, spawn_window)
            .add_systems(
                Update,
                (update_window, update_achievements)
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_window(mut commands: Commands) {
    commands
        .spawn((
            TrophyWindow,
            Node {
                position_type: PositionType::Absolute,
                // Left, under the unit frames: the right edge belongs to the minimap,
                // the quest tracker and the combat log.
                left: px(16),
                top: px(150),
                width: px(352),
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
            GlobalZIndex(30),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("~/trophies  (B to close)", 14.0), TextColor(DIM)));
                h.spawn((TrophyClose, Button, label("[x]", 14.0)));
            });
            c.spawn(Node {
                column_gap: px(6),
                ..default()
            })
            .with_children(|h| {
                for (tab, text) in [(false, "trophies"), (true, "achievements")] {
                    h.spawn((
                        TabButton(tab),
                        Button,
                        Node {
                            padding: UiRect::axes(px(8), px(2)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BorderColor::all(PANEL_EDGE),
                        children![label(text, 13.0)],
                    ));
                }
            });
            c.spawn((
                AchPane,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|a| {
                a.spawn((AchTitle, label("", 13.0), TextColor(GOLD)));
                a.spawn((
                    AchList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(1),
                        ..default()
                    },
                ));
                a.spawn(Node {
                    width: percent(100),
                    min_height: px(52),
                    padding: UiRect::all(px(6)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                })
                .insert((BorderColor::all(PANEL_EDGE), BackgroundColor(super::BAR_BG)))
                .with_children(|d| {
                    d.spawn((
                        AchDetail::Name,
                        label(ACH_HINT, 12.0),
                        TextColor(Color::WHITE),
                    ));
                    d.spawn((AchDetail::Text, label("", 12.0), TextColor(DIM)));
                });
            });
            c.spawn((
                TrophyPane,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn((
                    TrophyGrid,
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(4),
                        row_gap: px(4),
                        min_height: px(TILE),
                        ..default()
                    },
                ));
                c.spawn(Node {
                    width: percent(100),
                    min_height: px(64),
                    padding: UiRect::all(px(6)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                })
                .insert((BorderColor::all(PANEL_EDGE), BackgroundColor(super::BAR_BG)))
                .with_children(|d| {
                    for (kind, text, color) in [
                        (Detail::Name, HINT, Color::WHITE),
                        (Detail::Bonus, "", BONUS_GREEN),
                        (Detail::Desc, "", DIM),
                    ] {
                        d.spawn((
                            kind,
                            label(text, 12.0),
                            TextColor(color),
                            Node {
                                width: percent(100),
                                ..default()
                            },
                        ));
                    }
                });
                c.spawn((TrophyTotals, label("", 13.0), TextColor(BONUS_GREEN)));
            });
        });
}

/// The achievements tab: rows rebuilt when the unlocked list or the worn title changes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters
fn update_achievements(
    mut commands: Commands,
    open: Res<TrophiesOpen>,
    mut tab: ResMut<AchTab>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    tabs: Query<(&Interaction, &TabButton, &mut BorderColor), Without<AchRow>>,
    mut panes: ParamSet<(
        Single<&mut Node, With<AchPane>>,
        Single<&mut Node, With<TrophyPane>>,
    )>,
    list: Single<Entity, With<AchList>>,
    rows: Query<(&Interaction, &AchRow, &mut BorderColor), Without<TabButton>>,
    clicks: Query<(&Interaction, &AchRow), Changed<Interaction>>,
    mut title: Single<&mut Text, (With<AchTitle>, Without<AchDetail>)>,
    mut details: Query<(&AchDetail, &mut Text), Without<AchTitle>>,
    mut shown: Local<Option<(Vec<String>, String)>>,
) {
    let mut tabs = tabs;
    for (i, t, mut border) in &mut tabs {
        if *i == Interaction::Pressed {
            tab.0 = t.0;
        }
        *border = BorderColor::all(if t.0 == tab.0 { ACCENT } else { PANEL_EDGE });
    }
    let (ach, trophy) = if tab.0 {
        (Display::Flex, Display::None)
    } else {
        (Display::None, Display::Flex)
    };
    panes.p0().display = ach;
    panes.p1().display = trophy;
    if !open.0 || !tab.0 {
        return;
    }
    let Some(me) = sim.player().and_then(|p| sim.unit(p)) else {
        return;
    };
    let content = sim.content();
    let state = (me.achievements.clone(), me.title.clone());
    let worn = content
        .achievement(&me.title)
        .map_or_else(|| "none".to_string(), |a| format!("<{}>", a.title));
    set_text(
        &mut title,
        &format!(
            "Title: {worn}    ({}/{} secrets)",
            me.achievements.len(),
            content.world.achievements.len()
        ),
    );
    if shown.as_ref() != Some(&state) {
        commands.entity(*list).despawn_related::<Children>();
        for a in &content.world.achievements {
            let got = me.achievements.contains(&a.key);
            let worn = me.title == a.key;
            let text = if got {
                format!("{} {}  <{}>", if worn { "*" } else { " " }, a.name, a.title)
            } else {
                "  ??? (secret)".to_string()
            };
            commands.entity(*list).with_child((
                AchRow(a.key.clone()),
                Button,
                Node {
                    padding: UiRect::axes(px(4), px(1)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(Color::NONE),
                children![(
                    label(text, 12.0),
                    TextColor(if worn {
                        GOLD
                    } else if got {
                        Color::WHITE
                    } else {
                        LOCKED
                    })
                )],
            ));
        }
        *shown = Some(state);
    }
    let mut rows = rows;
    let mut hovered = None;
    for (i, row, mut border) in &mut rows {
        let on = *i != Interaction::None;
        *border = BorderColor::all(if on { ACCENT } else { Color::NONE });
        if on {
            hovered = content.achievement(&row.0);
        }
    }
    for (i, row) in &clicks {
        if *i == Interaction::Pressed && me.achievements.contains(&row.0) {
            // Clicking the worn title takes it off.
            let pick = if me.title == row.0 {
                String::new()
            } else {
                row.0.clone()
            };
            pending.0.push(Command::SetTitle { achievement: pick });
        }
    }
    let (name, text) = match hovered {
        Some(a) if me.achievements.contains(&a.key) => (a.name.clone(), a.text.clone()),
        Some(a) => ("Locked secret".to_string(), format!("Hint: {}", a.hint)),
        None => (ACH_HINT.to_string(), String::new()),
    };
    for (d, mut t) in &mut details {
        set_text(
            &mut t,
            match d {
                AchDetail::Name => &name,
                AchDetail::Text => &text,
            },
        );
    }
}

fn update_window(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    mut open: ResMut<TrophiesOpen>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut window: Single<&mut Node, With<TrophyWindow>>,
    grid: Single<Entity, With<TrophyGrid>>,
    mut tiles: Query<(&Interaction, &TrophyTile, &mut BorderColor)>,
    close: Query<&Interaction, (With<TrophyClose>, Changed<Interaction>)>,
    mut texts: Query<(&Detail, &mut Text), Without<TrophyTotals>>,
    mut totals: Single<&mut Text, With<TrophyTotals>>,
    mut shown: Local<Option<Vec<(String, u32)>>>,
) {
    if !paused.0 && binds.just(&keys, crate::keybinds::Action::Trophies) {
        open.0 = !open.0;
    }
    if close.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = false;
    }
    window.display = if open.0 { Display::Flex } else { Display::None };
    if !open.0 {
        return;
    }

    let inventory = sim.inventory();
    if shown.as_deref() != Some(inventory) {
        *shown = Some(inventory.to_vec());
        rebuild_grid(&mut commands, *grid, &*sim, &assets, &mut icons);
    }
    let sums = copy::bonus_text(&sim.item_totals());
    set_text(
        &mut totals,
        &if sums.is_empty() {
            "Total: nothing yet. Leadership is not impressed.".to_string()
        } else {
            format!("Total: {sums}")
        },
    );

    let mut hovered = None;
    for (interaction, tile, mut border) in &mut tiles {
        let on = *interaction != Interaction::None;
        *border = BorderColor::all(if on { ACCENT } else { PANEL_EDGE });
        if on {
            hovered = Some(tile.0.as_str());
        }
    }
    let (name, bonus, desc) = hovered
        .and_then(|n| sim.content().item(n).map(|d| (d, count_of(inventory, n))))
        .map_or_else(
            || (HINT.to_string(), String::new(), String::new()),
            |(d, copies)| {
                let mut bonus = copy::bonus_text(&d.bonus.totals(copies));
                if copies > ITEM_STACK_CAP {
                    bonus.push_str(&format!("  (stack cap {ITEM_STACK_CAP})"));
                }
                (
                    format!("{} x{copies}", d.name),
                    bonus,
                    d.description.clone(),
                )
            },
        );
    for (kind, mut text) in &mut texts {
        set_text(
            &mut text,
            match kind {
                Detail::Name => &name,
                Detail::Bonus => &bonus,
                Detail::Desc => &desc,
            },
        );
    }
}

fn count_of(inventory: &[(String, u32)], name: &str) -> u32 {
    inventory
        .iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, c)| *c)
}

fn rebuild_grid(
    commands: &mut Commands,
    grid: Entity,
    sim: &dyn SimRead,
    assets: &AssetServer,
    icons: &mut IconCache,
) {
    commands.entity(grid).despawn_related::<Children>();
    let inventory = sim.inventory();
    if inventory.is_empty() {
        commands.entity(grid).with_child((
            label("Empty. Kill something, loot it, expense it.", 12.0),
            TextColor(DIM),
        ));
    }
    for (name, copies) in inventory {
        let icon_key = sim
            .content()
            .item(name)
            .map_or("unknown", |d| d.icon.as_str());
        let icon = icons.get(assets, icon_key);
        let badge = if *copies >= ITEM_STACK_CAP {
            GOLD
        } else {
            Color::WHITE
        };
        commands.entity(grid).with_child((
            TrophyTile(name.clone()),
            Button,
            Node {
                width: px(TILE),
                height: px(TILE),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
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
                        display: if *copies > 1 {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        ..default()
                    },
                    label(format!("x{copies}"), 12.0),
                    TextColor(badge),
                ),
            ],
        ));
    }
}
