//! Party frames under the player frame: name, level, HP and mana of every other member of
//! the online room. Hidden offline.

use super::world::NetWorld;
use crate::combat::SimRead;
use crate::hud::{BAR_BG, DIM, HP_GREEN, MANA_BLUE, label, set_text};
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

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_party)
            .add_systems(Update, update_party);
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
