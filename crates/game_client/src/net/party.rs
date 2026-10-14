//! Party frames under the player frame: name, level, HP and mana of every other member of
//! the online room. Hidden offline. Clicking a frame targets that member (like F2-F5).

use super::world::NetWorld;
use crate::combat::{SimRead, SimUnit};
use crate::hud::{BAR_BG, DIM, HP_GREEN, IconCache, MANA_BLUE, TipSource, label, set_text};
use crate::targeting::{Target, entity_of_sim};
use bevy::prelude::*;
use combat_core::{MAX_PLAYERS, UnitState};

/// Below the player frame, its aura row and cast bar (see the bubble-free zone in yell.rs).
const PARTY_TOP: f32 = 156.0;
const PARTY_W: f32 = 190.0;

#[derive(Component)]
struct PartyRow(usize);
#[derive(Component)]
struct PartyName(usize);
#[derive(Component)]
struct PartyFill(usize, bool);
/// One small aura icon under a member's bars: (member slot, aura index).
#[derive(Component)]
struct PartyAura(usize, usize);

const PARTY_AURAS: usize = 6;
const PARTY_AURA_PX: f32 = 16.0;

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_party)
            .add_systems(Update, (update_party, update_party_auras, click_party));
    }
}

fn bar(p: &mut ChildSpawnerCommands, slot: usize, mana: bool) {
    p.spawn((
        Node {
            width: percent(100),
            height: px(if mana { 4.0 } else { 7.0 }),
            border_radius: BorderRadius::all(px(2)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(BAR_BG),
    ))
    .with_children(|b| {
        b.spawn((
            PartyFill(slot, mana),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(if mana { MANA_BLUE } else { HP_GREEN }),
        ));
    });
}

fn spawn_party(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(PARTY_TOP),
            left: px(12),
            width: px(PARTY_W),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        })
        .with_children(|root| {
            for slot in 0..MAX_PLAYERS - 1 {
                root.spawn((
                    PartyRow(slot),
                    Button,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        display: Display::None,
                        ..default()
                    },
                ))
                .with_children(|r| {
                    r.spawn((PartyName(slot), label("", 12.0), TextColor::WHITE));
                    bar(r, slot, false);
                    bar(r, slot, true);
                    r.spawn(Node {
                        column_gap: px(3),
                        ..default()
                    })
                    .with_children(|auras| {
                        for i in 0..PARTY_AURAS {
                            auras.spawn((
                                PartyAura(slot, i),
                                Button,
                                TipSource {
                                    tag: "party-aura",
                                    ..default()
                                },
                                ImageNode::default(),
                                Node {
                                    width: px(PARTY_AURA_PX),
                                    height: px(PARTY_AURA_PX),
                                    border: UiRect::all(px(1)),
                                    border_radius: BorderRadius::all(px(2)),
                                    display: Display::None,
                                    ..default()
                                },
                                BorderColor::all(DIM),
                            ));
                        }
                    });
                });
            }
        });
}

/// Members other than the local player, in join order.
fn members(world: &NetWorld) -> Vec<&UnitState> {
    world
        .players()
        .iter()
        .filter(|id| **id != world.you)
        .filter_map(|id| world.unit(*id))
        .collect()
}

fn click_party(
    net: Option<Res<NetWorld>>,
    rows: Query<(&PartyRow, &Interaction), Changed<Interaction>>,
    units: Query<(Entity, &SimUnit)>,
    mut target: ResMut<Target>,
) {
    let Some(world) = net.as_deref() else { return };
    for (PartyRow(slot), i) in &rows {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(e) = members(world)
            .get(*slot)
            .and_then(|u| entity_of_sim(&units, u.id))
        {
            target.0 = Some(e);
        }
    }
}

fn fraction(cur: u32, max: u32) -> f32 {
    if max == 0 {
        0.0
    } else {
        (cur as f32 / max as f32).clamp(0.0, 1.0)
    }
}

fn update_party(
    net: Option<Res<NetWorld>>,
    mut rows: Query<(&PartyRow, &mut Node), (Without<PartyFill>,)>,
    mut names: Query<(&PartyName, &mut Text, &mut TextColor)>,
    mut fills: Query<(&PartyFill, &mut Node), Without<PartyRow>>,
) {
    let list = net.as_deref().map(members).unwrap_or_default();
    for (PartyRow(slot), mut node) in &mut rows {
        let want = if *slot < list.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (PartyName(slot), mut text, mut color) in &mut names {
        let Some(u) = list.get(*slot) else { continue };
        let line = if u.alive {
            format!("{}  {}", u.name, u.level)
        } else {
            format!("{}  {}  (dead)", u.name, u.level)
        };
        set_text(&mut text, &line);
        color.0 = if u.alive { Color::WHITE } else { DIM };
    }
    for (PartyFill(slot, mana), mut node) in &mut fills {
        let Some(u) = list.get(*slot) else { continue };
        let k = if *mana {
            fraction(u.mana, u.max_mana)
        } else {
            fraction(u.hp, u.max_hp)
        };
        let w = percent(k * 100.0);
        if node.width != w {
            node.width = w;
        }
    }
}

fn update_party_auras(
    net: Option<Res<NetWorld>>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut slots: Query<(
        &PartyAura,
        &mut Node,
        &mut ImageNode,
        &mut BorderColor,
        &mut TipSource,
    )>,
) {
    let list = net.as_deref().map(members).unwrap_or_default();
    for (PartyAura(slot, i), mut node, mut img, mut border, mut tip) in &mut slots {
        let aura = list.get(*slot).and_then(|u| u.auras.get(*i));
        let want = if aura.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
        tip.aura = aura.cloned();
        let Some(a) = aura else { continue };
        let handle = icons.get(&assets, &a.icon);
        if img.image != handle {
            img.image = handle;
        }
        *border = BorderColor::all(if a.harmful {
            Color::srgb(0.8, 0.15, 0.15)
        } else {
            Color::srgb(0.3, 0.75, 0.35)
        });
    }
}
