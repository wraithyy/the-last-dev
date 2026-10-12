//! Player and target unit frames, cast bars and aura icon rows.

use super::{
    BAR_BG, BRONZE, GOLD, HP_GREEN, IconCache, MANA_BLUE, fmt_secs, inset_line, label, set_text,
};
use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::config::*;
use crate::player::Player;
use crate::targeting::Target;
use bevy::prelude::*;
use combat_core::{AuraView, CastError, CastView, EntityId, Event};

const FRAME_H: f32 = 92.0;
const FRAME_TOP: f32 = 12.0;
const FRAME_GAP: f32 = 14.0;
const CASTING: Color = Color::srgb(1.0, 0.72, 0.1);
const UNINTERRUPTIBLE: Color = Color::srgb(0.55, 0.55, 0.58);
const CAST_OK: Color = Color::srgb(0.2, 0.8, 0.25);
const CAST_FAIL: Color = Color::srgb(0.85, 0.15, 0.12);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Who {
    Player,
    Target,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Pool {
    Hp,
    Mana,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Level,
    Initial,
    Hp,
    Mana,
}
#[derive(Component)]
struct FrameRoot(Who);
#[derive(Component)]
struct TextOf(Who, Field);
#[derive(Component)]
struct Fill(Who, Pool);
#[derive(Component)]
struct PoolRow(Who, Pool);
#[derive(Component)]
struct Disc(Who);

#[derive(Component)]
struct CastRoot(Who);
#[derive(Component)]
struct CastFill(Who);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum CastField {
    Name,
    Time,
}
#[derive(Component)]
struct CastText(Who, CastField);
#[derive(Component)]
struct CastShield;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Row {
    PlayerAuras,
    TargetDebuffs,
}
#[derive(Component)]
struct AuraSlot(Row, usize);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Icon,
    Time,
    Absorb,
}
#[derive(Component)]
struct AuraPart(Row, usize, Part);

/// What the player cast bar shows after a cast ended (the sim has no cast left to show).
#[derive(Resource, Default)]
struct CastFlash {
    left: f32,
    text: String,
    color: Option<Color>,
    was_casting: bool,
}

pub struct FramesPlugin;

impl Plugin for FramesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastFlash>()
            .add_systems(Startup, spawn_frames)
            .add_systems(
                Update,
                (
                    update_frames,
                    collect_cast_flash,
                    update_cast_bars,
                    update_auras,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

fn spawn_bar(parent: &mut ChildSpawnerCommands, who: Who, pool: Pool) {
    let (color, field) = match pool {
        Pool::Hp => (HP_GREEN, Field::Hp),
        Pool::Mana => (MANA_BLUE, Field::Mana),
    };
    parent
        .spawn((
            PoolRow(who, pool),
            Node {
                height: px(FRAME_BAR_H),
                width: percent(100),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(BAR_BG),
        ))
        .with_children(|bar| {
            bar.spawn((
                Fill(who, pool),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: percent(100),
                    ..default()
                },
                BackgroundColor(color),
            ));
            bar.spawn((TextOf(who, field), label("", 12.0), TextColor::WHITE));
        });
}

fn spawn_frame(commands: &mut Commands, who: Who, left: f32) {
    commands
        .spawn((
            FrameRoot(who),
            Node {
                position_type: PositionType::Absolute,
                top: px(FRAME_TOP),
                left: px(left),
                width: px(FRAME_W),
                height: px(FRAME_H),
                padding: UiRect::all(px(8)),
                column_gap: px(8),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(12)),
                align_items: AlignItems::Center,
                display: if who == Who::Player {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(super::PANEL),
        ))
        .with_children(|frame| {
            frame.spawn(inset_line(12.0));
            frame
                .spawn((
                    Disc(who),
                    Node {
                        width: px(PORTRAIT_SIZE),
                        height: px(PORTRAIT_SIZE),
                        flex_shrink: 0.0,
                        border: UiRect::all(px(3)),
                        border_radius: BorderRadius::MAX,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(GOLD),
                    BackgroundColor(Color::srgb(0.2, 0.3, 0.7)),
                ))
                .with_children(|d| {
                    d.spawn((TextOf(who, Field::Initial), label("", 30.0)));
                });
            frame
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                })
                .with_children(|body| {
                    body.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((TextOf(who, Field::Name), label("", 14.0), TextColor::WHITE));
                        row.spawn((
                            Node {
                                width: px(22),
                                height: px(22),
                                flex_shrink: 0.0,
                                border: UiRect::all(px(1)),
                                border_radius: BorderRadius::MAX,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BorderColor::all(GOLD),
                            BackgroundColor(BRONZE),
                            children![(
                                TextOf(who, Field::Level),
                                label("", 12.0),
                                TextColor::WHITE
                            )],
                        ));
                    });
                    spawn_bar(body, who, Pool::Hp);
                    spawn_bar(body, who, Pool::Mana);
                });
        });
}

fn spawn_cast_bar(
    commands: &mut Commands,
    who: Who,
    node: Node,
    icons: &mut IconCache,
    assets: &AssetServer,
) {
    commands
        .spawn((
            CastRoot(who),
            Node {
                position_type: PositionType::Absolute,
                height: px(CAST_BAR_H),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(4)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                display: Display::None,
                ..node
            },
            BorderColor::all(GOLD),
            BackgroundColor(BAR_BG),
        ))
        .with_children(|bar| {
            bar.spawn((
                CastFill(who),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: percent(0),
                    ..default()
                },
                BackgroundColor(CASTING),
            ));
            bar.spawn((
                CastText(who, CastField::Name),
                label("", 13.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6),
                    ..default()
                },
            ));
            bar.spawn((
                CastText(who, CastField::Time),
                label("", 12.0),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(6),
                    ..default()
                },
            ));
            if who == Who::Target {
                bar.spawn((
                    CastShield,
                    ImageNode::new(icons.get(assets, "guardrails")),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(-2),
                        width: px(CAST_BAR_H),
                        height: px(CAST_BAR_H),
                        display: Display::None,
                        ..default()
                    },
                ));
            }
        });
}

fn spawn_aura_row(
    commands: &mut Commands,
    row: Row,
    node: Node,
    icons: &mut IconCache,
    assets: &AssetServer,
) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            column_gap: px(4),
            ..node
        })
        .with_children(|r| {
            for i in 0..AURA_SLOTS {
                r.spawn((
                    AuraSlot(row, i),
                    Node {
                        width: px(AURA_ICON),
                        height: px(AURA_ICON),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(3)),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(GOLD),
                    BackgroundColor(BAR_BG),
                ))
                .with_children(|slot| {
                    slot.spawn((
                        AuraPart(row, i, Part::Icon),
                        ImageNode::new(icons.get(assets, "code_smell")),
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                    ));
                    slot.spawn((
                        AuraPart(row, i, Part::Time),
                        label("", 11.0),
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(-15),
                            width: percent(100),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        TextLayout::justify(Justify::Center),
                    ));
                    slot.spawn((
                        AuraPart(row, i, Part::Absorb),
                        label("", 11.0),
                        TextColor(Color::srgb(1.0, 0.9, 0.4)),
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(0),
                            right: px(1),
                            ..default()
                        },
                    ));
                });
            }
        });
}

fn spawn_frames(mut commands: Commands, assets: Res<AssetServer>, mut icons: ResMut<IconCache>) {
    spawn_frame(&mut commands, Who::Player, 12.0);
    let target_left = 12.0 + FRAME_W + FRAME_GAP;
    spawn_frame(&mut commands, Who::Target, target_left);

    let below = FRAME_TOP + FRAME_H + 6.0;
    spawn_cast_bar(
        &mut commands,
        Who::Target,
        Node {
            top: px(below),
            left: px(target_left + 24.0),
            width: px(FRAME_W - 24.0),
            ..default()
        },
        &mut icons,
        &assets,
    );
    spawn_aura_row(
        &mut commands,
        Row::TargetDebuffs,
        Node {
            top: px(below + CAST_BAR_H + 8.0),
            left: px(target_left),
            ..default()
        },
        &mut icons,
        &assets,
    );
    spawn_aura_row(
        &mut commands,
        Row::PlayerAuras,
        Node {
            top: px(FRAME_TOP),
            right: px(12),
            flex_direction: FlexDirection::RowReverse,
            ..default()
        },
        &mut icons,
        &assets,
    );
    // Player cast bar sits above the action bar, centred.
    spawn_cast_bar(
        &mut commands,
        Who::Player,
        Node {
            bottom: px(120),
            left: percent(50),
            margin: UiRect::left(px(-CAST_BAR_W / 2.0)),
            width: px(CAST_BAR_W),
            ..default()
        },
        &mut icons,
        &assets,
    );
}

fn tracked(
    who: Who,
    sim: &CombatSim,
    target: &Target,
    units: &Query<&SimUnit>,
) -> Option<EntityId> {
    match who {
        Who::Player => sim.0.player(),
        Who::Target => target.0.and_then(|e| units.get(e).ok()).map(|s| s.0),
    }
}

fn update_frames(
    sim: Res<CombatSim>,
    target: Res<Target>,
    units: Query<&SimUnit>,
    mut roots: Query<(&FrameRoot, &mut Node)>,
    mut rows: Query<(&PoolRow, &mut Node), Without<FrameRoot>>,
    mut fills: Query<(&Fill, &mut Node), (Without<FrameRoot>, Without<PoolRow>)>,
    mut texts: Query<(&TextOf, &mut Text, &mut TextColor)>,
    mut discs: Query<(&Disc, &mut BackgroundColor)>,
) {
    for (root, mut node) in &mut roots {
        node.display = if tracked(root.0, &sim, &target, &units)
            .and_then(|id| sim.0.unit(id))
            .is_some()
        {
            Display::Flex
        } else {
            Display::None
        };
    }
    let state = |who| tracked(who, &sim, &target, &units).and_then(|id| sim.0.unit(id));
    for (row, mut node) in &mut rows {
        let has_pool = state(row.0).is_some_and(|u| row.1 == Pool::Hp || u.max_mana > 0);
        node.display = if has_pool {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (fill, mut node) in &mut fills {
        let Some(u) = state(fill.0) else { continue };
        let (cur, max) = match fill.1 {
            Pool::Hp => (u.hp, u.max_hp),
            Pool::Mana => (u.mana, u.max_mana),
        };
        let pct = if max == 0 {
            0.0
        } else {
            100.0 * cur as f32 / max as f32
        };
        node.width = percent(pct.clamp(0.0, 100.0));
    }
    for (TextOf(who, field), mut text, mut color) in &mut texts {
        let Some(u) = state(*who) else { continue };
        match field {
            Field::Name => {
                set_text(&mut text, &u.name);
                color.0 = if !u.alive {
                    Color::srgb(0.6, 0.6, 0.6)
                } else if *who == Who::Player {
                    Color::WHITE
                } else if u.hostile {
                    Color::srgb(1.0, 0.25, 0.2)
                } else {
                    Color::srgb(0.35, 1.0, 0.4)
                };
            }
            Field::Level => set_text(&mut text, &u.level.to_string()),
            Field::Initial => {
                set_text(&mut text, &u.name.chars().next().unwrap_or('?').to_string())
            }
            Field::Hp if !u.alive => set_text(
                &mut text,
                if u.lootable {
                    "Dead - lootable"
                } else {
                    "Dead"
                },
            ),
            Field::Hp => set_text(&mut text, &format!("{} / {}", u.hp, u.max_hp)),
            Field::Mana => set_text(&mut text, &format!("{} / {}", u.mana, u.max_mana)),
        }
    }
    for (Disc(who), mut bg) in &mut discs {
        let Some(u) = state(*who) else { continue };
        bg.0 = match (*who, u.hostile) {
            (Who::Player, _) => Color::srgb(0.2, 0.32, 0.75),
            (_, true) => Color::srgb(0.5, 0.14, 0.2),
            (_, false) => Color::srgb(0.2, 0.5, 0.3),
        };
    }
}

fn collect_cast_flash(
    mut events: MessageReader<CombatEvent>,
    sim: Res<CombatSim>,
    mut flash: ResMut<CastFlash>,
    time: Res<Time>,
) {
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    let Some(me) = sim.0.player() else { return };
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastSucceeded { caster, spell, .. } if *caster == me => {
                if let Some(def) = sim.0.content().spell(*spell)
                    && def.cast_ms > 0
                {
                    flash.text.clone_from(&def.name);
                    flash.color = Some(CAST_OK);
                    flash.left = CAST_FLASH_SECS;
                }
            }
            Event::CastInterrupted { caster, .. } if *caster == me => {
                "Interrupted".clone_into(&mut flash.text);
                flash.color = Some(CAST_FAIL);
                flash.left = CAST_FLASH_SECS * 2.0;
            }
            // A failure with a cast in progress means that cast was lost (e.g. moved).
            Event::CastFailed { caster, reason, .. } if *caster == me && flash.was_casting => {
                let _: CastError = *reason;
                "Interrupted".clone_into(&mut flash.text);
                flash.color = Some(CAST_FAIL);
                flash.left = CAST_FLASH_SECS * 2.0;
            }
            _ => {}
        }
    }
    flash.was_casting = sim.0.unit(me).is_some_and(|u| u.cast.is_some());
}

struct BarView {
    fill: f32,
    color: Color,
    name: String,
    time: String,
    shield: bool,
}

fn cast_view(sim: &CombatSim, cast: CastView, remaining_only: bool) -> BarView {
    let name = sim
        .0
        .content()
        .spell(cast.spell)
        .map_or_else(String::new, |d| d.name.clone());
    let total = cast.total_ms.max(1);
    let secs = |ms: u32| ms as f32 / 1000.0;
    BarView {
        fill: (cast.elapsed_ms as f32 / total as f32).clamp(0.0, 1.0),
        color: if cast.interruptible {
            CASTING
        } else {
            UNINTERRUPTIBLE
        },
        name,
        time: if remaining_only {
            format!("{:.1}", secs(total.saturating_sub(cast.elapsed_ms)))
        } else {
            format!("{:.1} / {:.1}", secs(cast.elapsed_ms), secs(cast.total_ms))
        },
        shield: !cast.interruptible,
    }
}

fn update_cast_bars(
    sim: Res<CombatSim>,
    target: Res<Target>,
    flash: Res<CastFlash>,
    units: Query<&SimUnit>,
    mut roots: Query<(&CastRoot, &mut Node)>,
    mut fills: Query<(&CastFill, &mut Node, &mut BackgroundColor), Without<CastRoot>>,
    mut texts: Query<(&CastText, &mut Text)>,
    mut shield: Query<&mut Node, (With<CastShield>, Without<CastRoot>, Without<CastFill>)>,
) {
    let view = |who: Who| -> Option<BarView> {
        let id = tracked(who, &sim, &target, &units)?;
        let u = sim.0.unit(id)?;
        match (u.cast, who) {
            (Some(c), Who::Player) => Some(cast_view(&sim, c, false)),
            (Some(c), Who::Target) if u.alive => Some(cast_view(&sim, c, true)),
            (None, Who::Player) if flash.left > 0.0 => flash.color.map(|color| BarView {
                fill: 1.0,
                color,
                name: flash.text.clone(),
                time: String::new(),
                shield: false,
            }),
            _ => None,
        }
    };
    let (player, tgt) = (view(Who::Player), view(Who::Target));
    let pick = |who| if who == Who::Player { &player } else { &tgt };
    for (root, mut node) in &mut roots {
        node.display = if pick(root.0).is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (CastFill(who), mut node, mut bg) in &mut fills {
        if let Some(v) = pick(*who) {
            node.width = percent(v.fill * 100.0);
            bg.0 = v.color;
        }
    }
    for (CastText(who, field), mut text) in &mut texts {
        if let Some(v) = pick(*who) {
            set_text(
                &mut text,
                if *field == CastField::Name {
                    &v.name
                } else {
                    &v.time
                },
            );
        }
    }
    for mut node in &mut shield {
        node.display = if tgt.as_ref().is_some_and(|v| v.shield) {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn update_auras(
    sim: Res<CombatSim>,
    target: Res<Target>,
    units: Query<&SimUnit>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut slots: Query<(&AuraSlot, &mut Node, &mut BorderColor)>,
    mut images: Query<(&AuraPart, &mut ImageNode)>,
    mut texts: Query<(&AuraPart, &mut Text)>,
) {
    let auras_of = |row: Row| -> Vec<AuraView> {
        match row {
            Row::PlayerAuras => {
                let Some(u) = sim.0.unit(player.0) else {
                    return Vec::new();
                };
                let (mut buffs, debuffs): (Vec<_>, Vec<_>) =
                    u.auras.iter().cloned().partition(|a| !a.harmful);
                buffs.extend(debuffs);
                buffs
            }
            Row::TargetDebuffs => target
                .0
                .and_then(|e| units.get(e).ok())
                .and_then(|s| sim.0.unit(s.0))
                .map(|u| u.auras.iter().filter(|a| a.harmful).cloned().collect())
                .unwrap_or_default(),
        }
    };
    let rows = [
        (Row::PlayerAuras, auras_of(Row::PlayerAuras)),
        (Row::TargetDebuffs, auras_of(Row::TargetDebuffs)),
    ];
    let get = |row: Row, i: usize| {
        rows.iter()
            .find(|(r, _)| *r == row)
            .and_then(|(_, v)| v.get(i))
    };
    for (AuraSlot(row, i), mut node, mut border) in &mut slots {
        let aura = get(*row, *i);
        node.display = if aura.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if let Some(a) = aura {
            *border = BorderColor::all(if a.harmful {
                Color::srgb(0.8, 0.15, 0.15)
            } else {
                Color::srgb(0.3, 0.75, 0.35)
            });
        }
    }
    for (AuraPart(row, i, _), mut img) in &mut images {
        if let Some(a) = get(*row, *i) {
            let handle = icons.get(&assets, &a.icon);
            if img.image != handle {
                img.image = handle;
            }
        }
    }
    for (AuraPart(row, i, part), mut text) in &mut texts {
        let Some(a) = get(*row, *i) else { continue };
        match part {
            Part::Time => set_text(&mut text, &fmt_secs(a.remaining_ms)),
            Part::Absorb => set_text(
                &mut text,
                &a.absorb_remaining
                    .map_or_else(String::new, |n| n.to_string()),
            ),
            Part::Icon => {}
        }
    }
}
