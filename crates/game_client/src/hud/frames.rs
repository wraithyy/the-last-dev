//! Player and target unit frames, cast bars and aura icon rows.

use super::{
    ACCENT_WARM, BAR_BG, DIM, HP_GREEN, IconCache, MANA_BLUE, PANEL_BOTTOM, PANEL_EDGE, PANEL_TOP,
    fmt_secs, gloss, label, set_text, vgrad,
};
use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit};
use crate::config::*;
use crate::copy;
use crate::player::Player;
use crate::targeting::Target;
use bevy::prelude::*;
use combat_core::{AuraView, CastError, CastView, EntityId, Event};

const FRAME_H: f32 = 50.0;
// Leaves room for the controls hint line above the frames.
const FRAME_TOP: f32 = 26.0;
const FRAME_GAP: f32 = 14.0;
const AVATAR_RIM: Color = Color::srgb(0.55, 0.62, 0.75);
const CASTING: Color = Color::srgb(1.0, 0.72, 0.1);
const UNINTERRUPTIBLE: Color = Color::srgb(0.55, 0.55, 0.58);
const CAST_OK: Color = Color::srgb(0.2, 0.8, 0.25);
const CAST_FAIL: Color = Color::srgb(0.85, 0.15, 0.12);
/// Bars ease toward the sim value at this exponential rate (1/s) instead of jumping.
const BAR_TWEEN_RATE: f32 = 10.0;
/// Space the spell icon takes left of a cast bar.
const CAST_ICON_GAP: f32 = 6.0;
const CAST_SEGMENTS: usize = 10;

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
    Title,
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
/// Displayed bar fraction; `owner` snaps the bar when the tracked unit changes.
#[derive(Component, Default)]
struct Smooth {
    shown: f32,
    owner: Option<EntityId>,
}
#[derive(Component)]
struct PoolRow(Who, Pool);
#[derive(Component)]
struct Disc(Who);
#[derive(Component)]
struct BossTag(Who);
/// "PROD" (boss, with its phase) or "ELITE" (mini-boss).
#[derive(Component)]
struct BossTagText(Who);

/// The frame disc flashes when the tracked boss changes phase.
#[derive(Resource, Default)]
struct PhaseFlash {
    unit: Option<EntityId>,
    left: f32,
}
const PHASE_FLASH_SECS: f32 = 0.9;

#[derive(Component)]
struct CastRoot(Who);
#[derive(Component)]
struct CastFill {
    who: Who,
    color: Color,
}
#[derive(Component)]
struct CastSpark(Who);
#[derive(Component)]
struct CastIcon(Who);
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
    Stacks,
}
#[derive(Component)]
struct AuraPart(Row, usize, Part);

/// What the player cast bar shows after a cast ended (the sim has no cast left to show).
#[derive(Resource, Default)]
struct CastFlash {
    left: f32,
    text: String,
    icon: String,
    color: Option<Color>,
    was_casting: bool,
}

pub struct FramesPlugin;

impl Plugin for FramesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastFlash>()
            .init_resource::<PhaseFlash>()
            .add_systems(Startup, spawn_frames)
            .add_systems(Update, reset_phase_flash_on_restart)
            .add_systems(
                Update,
                (
                    collect_phase_flash,
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

fn glossy(color: Color) -> BackgroundGradient {
    vgrad(
        color.mix(&Color::WHITE, 0.3),
        color.mix(&Color::BLACK, 0.45),
    )
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
            BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.6)),
            // Soft halo keeps the frameless bars readable over bright parts of the scene.
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), px(0), px(1), px(0), px(4)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Fill(who, pool),
                Smooth::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: percent(100),
                    ..default()
                },
                glossy(color),
            ));
            bar.spawn(gloss(0.18));
            bar.spawn((TextOf(who, field), label("", 10.0), TextColor::WHITE));
        });
}

/// Frameless unit frame: square avatar, name with an optional tag, title and two thin bars.
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
                column_gap: px(8),
                align_items: AlignItems::Center,
                display: if who == Who::Player {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
        ))
        .with_children(|frame| {
            frame
                .spawn((
                    Disc(who),
                    Node {
                        width: px(PORTRAIT_SIZE),
                        height: px(PORTRAIT_SIZE),
                        flex_shrink: 0.0,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(8)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(AVATAR_RIM),
                    BackgroundColor(Color::srgb(0.2, 0.3, 0.7)),
                    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), px(0), px(1), px(0), px(4)),
                ))
                .with_children(|d| {
                    d.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(45),
                            top: px(0),
                            border_radius: BorderRadius::top(px(6)),
                            ..default()
                        },
                        vgrad(
                            Color::srgba(1.0, 1.0, 1.0, 0.22),
                            Color::srgba(1.0, 1.0, 1.0, 0.0),
                        ),
                    ));
                    d.spawn((TextOf(who, Field::Initial), label("", 22.0)));
                    d.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            right: px(-6),
                            bottom: px(-6),
                            min_width: px(17),
                            height: px(17),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(4)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(PANEL_EDGE),
                        vgrad(PANEL_TOP, PANEL_BOTTOM),
                        children![(TextOf(who, Field::Level), label("", 10.0), TextColor::WHITE)],
                    ));
                });
            frame
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                })
                .with_children(|body| {
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        min_width: px(0),
                        ..default()
                    })
                    .with_children(|names| {
                        names
                            .spawn(Node {
                                column_gap: px(6),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|row| {
                                row.spawn((
                                    TextOf(who, Field::Name),
                                    label("", 15.0),
                                    TextColor::WHITE,
                                ));
                                row.spawn((
                                    BossTag(who),
                                    Node {
                                        padding: UiRect::axes(px(4), px(0)),
                                        border: UiRect::all(px(1)),
                                        border_radius: BorderRadius::all(px(3)),
                                        display: Display::None,
                                        ..default()
                                    },
                                    BorderColor::all(ACCENT_WARM),
                                    vgrad(
                                        Color::srgb(0.4, 0.08, 0.06),
                                        Color::srgb(0.16, 0.03, 0.03),
                                    ),
                                    children![(
                                        BossTagText(who),
                                        label("PROD", 9.0),
                                        TextColor(ACCENT_WARM)
                                    )],
                                ));
                            });
                        names.spawn((TextOf(who, Field::Title), label("", 10.0), TextColor(DIM)));
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
                display: Display::None,
                ..node
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-(CAST_BAR_H + CAST_ICON_GAP)),
                    width: px(CAST_BAR_H),
                    height: px(CAST_BAR_H),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(BAR_BG),
                children![(
                    CastIcon(who),
                    ImageNode::new(icons.get(assets, "code_review")),
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                )],
            ));
            root.spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                vgrad(Color::srgb(0.02, 0.03, 0.05), Color::srgb(0.08, 0.09, 0.12)),
                super::drop_shadow(),
            ))
            .with_children(|bar| {
                bar.spawn((
                    CastFill {
                        who,
                        color: CASTING,
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        top: px(0),
                        bottom: px(0),
                        width: percent(0),
                        ..default()
                    },
                    glossy(CASTING),
                ));
                bar.spawn(gloss(0.25));
                // Segment ticks give the smooth bar a CLI progress "[=====>   ]" rhythm.
                for k in 1..CAST_SEGMENTS {
                    bar.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(100.0 * k as f32 / CAST_SEGMENTS as f32),
                            width: px(1),
                            top: px(0),
                            bottom: px(0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                    ));
                }
                bar.spawn((
                    CastSpark(who),
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(-4),
                        bottom: px(-4),
                        left: percent(0),
                        margin: UiRect::left(px(-5)),
                        width: px(10),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundGradient::from(LinearGradient::new(
                        LinearGradient::TO_RIGHT,
                        vec![
                            ColorStop::new(Color::srgba(1.0, 0.95, 0.7, 0.0), percent(0)),
                            ColorStop::new(Color::srgba(1.0, 0.97, 0.8, 0.95), percent(50)),
                            ColorStop::new(Color::srgba(1.0, 0.95, 0.7, 0.0), percent(100)),
                        ],
                    )),
                ));
                let name_left = if who == Who::Target {
                    CAST_BAR_H + 4.0
                } else {
                    8.0
                };
                bar.spawn((
                    CastText(who, CastField::Name),
                    label("", 12.0),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(name_left),
                        ..default()
                    },
                ));
                bar.spawn((
                    CastText(who, CastField::Time),
                    label("", 12.0),
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(8),
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
                    BorderColor::all(PANEL_EDGE),
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
                        label("", 10.0),
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
                        AuraPart(row, i, Part::Stacks),
                        label("", 11.0),
                        TextColor(Color::WHITE),
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(0),
                            right: px(1),
                            ..default()
                        },
                    ));
                    slot.spawn((
                        AuraPart(row, i, Part::Absorb),
                        label("", 10.0),
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
            left: px(target_left + 28.0),
            width: px(FRAME_W - 28.0),
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

fn collect_phase_flash(
    time: Res<Time>,
    mut events: MessageReader<CombatEvent>,
    mut flash: ResMut<PhaseFlash>,
) {
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    for CombatEvent(ev) in events.read() {
        if let Event::PhaseChanged { unit, phase } = ev
            && *phase > 1
        {
            flash.unit = Some(*unit);
            flash.left = PHASE_FLASH_SECS;
        }
    }
}

fn update_frames(
    sim: Res<CombatSim>,
    target: Res<Target>,
    time: Res<Time>,
    flash: Res<PhaseFlash>,
    units: Query<&SimUnit>,
    mut roots: Query<(&FrameRoot, &mut Node)>,
    mut rows: Query<(&PoolRow, &mut Node, &mut Visibility), Without<FrameRoot>>,
    mut fills: Query<
        (&Fill, &mut Smooth, &mut Node),
        (Without<FrameRoot>, Without<PoolRow>, Without<BossTag>),
    >,
    mut tags: Query<
        (&BossTag, &mut Node, &mut Visibility),
        (Without<FrameRoot>, Without<PoolRow>, Without<Fill>),
    >,
    mut texts: Query<(&TextOf, &mut Text, &mut TextColor), Without<BossTagText>>,
    mut tag_texts: Query<(&BossTagText, &mut Text)>,
    mut discs: Query<(&Disc, &mut BackgroundColor, &mut BorderColor)>,
) {
    let state = |who| tracked(who, &sim, &target, &units).and_then(|id| sim.0.unit(id));
    let boss = |who| state(who).is_some_and(|u| u.boss || copy::is_elite(&u.key));
    for (BossTagText(who), mut text) in &mut tag_texts {
        let Some(u) = state(*who) else { continue };
        let tag = match (u.boss, u.phase) {
            (true, p) if p > 1 => format!("PROD P{p}"),
            (true, _) => "PROD".to_string(),
            (false, _) => "ELITE".to_string(),
        };
        set_text(&mut text, &tag);
    }
    for (root, mut node) in &mut roots {
        node.display = if state(root.0).is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (tag, mut node, mut vis) in &mut tags {
        let shown = boss(tag.0);
        node.display = if shown { Display::Flex } else { Display::None };
        vis.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (row, mut node, mut vis) in &mut rows {
        let has_pool = state(row.0).is_some_and(|u| row.1 == Pool::Hp || u.max_mana > 0);
        node.display = if has_pool {
            Display::Flex
        } else {
            Display::None
        };
        // Gradient children of a Display::None node keep drawing with their last layout otherwise.
        vis.set_if_neq(if has_pool {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let ease = 1.0 - (-BAR_TWEEN_RATE * time.delta_secs()).exp();
    for (fill, mut smooth, mut node) in &mut fills {
        let Some(u) = state(fill.0) else { continue };
        let (cur, max) = match fill.1 {
            Pool::Hp => (u.hp, u.max_hp),
            Pool::Mana => (u.mana, u.max_mana),
        };
        let goal = if max == 0 {
            0.0
        } else {
            (cur as f32 / max as f32).clamp(0.0, 1.0)
        };
        // Retargeting must not sweep the bar from the previous unit's value.
        if smooth.owner != Some(u.id) {
            smooth.owner = Some(u.id);
            smooth.shown = goal;
        } else {
            smooth.shown += (goal - smooth.shown) * ease;
            if (goal - smooth.shown).abs() < 0.001 {
                smooth.shown = goal;
            }
        }
        let w = percent(smooth.shown * 100.0);
        if node.width != w {
            node.width = w;
        }
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
            Field::Title => set_text(
                &mut text,
                &sim.0
                    .content()
                    .unit_def(&u.key)
                    .filter(|d| !d.title.is_empty())
                    .map_or_else(String::new, |d| format!("<{}>", d.title)),
            ),
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
    for (Disc(who), mut bg, mut rim) in &mut discs {
        let Some(u) = state(*who) else { continue };
        let flashing = flash.left > 0.0 && flash.unit == Some(u.id);
        let want = BorderColor::all(if flashing {
            Color::WHITE
        } else if u.boss || copy::is_elite(&u.key) {
            ACCENT_WARM
        } else {
            AVATAR_RIM
        });
        if *rim != want {
            *rim = want;
        }
        let base = match (*who, u.hostile) {
            (Who::Player, _) => Color::srgb(0.2, 0.32, 0.75),
            (_, true) => Color::srgb(0.5, 0.14, 0.2),
            (_, false) => Color::srgb(0.2, 0.5, 0.3),
        };
        // A pulsing white-orange flash that fades over PHASE_FLASH_SECS.
        bg.0 = if flashing {
            let k = flash.left / PHASE_FLASH_SECS;
            let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 18.0).sin();
            base.mix(&ACCENT_WARM.mix(&Color::WHITE, pulse), k)
        } else {
            base
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
                    flash.icon.clone_from(&def.icon);
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
    let casting = sim.0.unit(me).and_then(|u| u.cast);
    if let Some(def) = casting.and_then(|c| sim.0.content().spell(c.spell))
        && flash.icon != def.icon
    {
        flash.icon.clone_from(&def.icon);
    }
    flash.was_casting = casting.is_some();
}

struct BarView {
    fill: f32,
    color: Color,
    name: String,
    icon: String,
    time: String,
    shield: bool,
    /// The spark only belongs on a bar that is still filling.
    spark: bool,
}

fn cast_view(sim: &CombatSim, cast: CastView, remaining_only: bool) -> BarView {
    let (name, icon) = sim
        .0
        .content()
        .spell(cast.spell)
        .map_or_else(Default::default, |d| (d.name.clone(), d.icon.clone()));
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
        icon,
        spark: true,
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
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut roots: Query<(&CastRoot, &mut Node, &mut Visibility)>,
    mut fills: Query<(&mut CastFill, &mut Node, &mut BackgroundGradient), Without<CastRoot>>,
    mut sparks: Query<(&CastSpark, &mut Node), (Without<CastRoot>, Without<CastFill>)>,
    mut icon_nodes: Query<(&CastIcon, &mut ImageNode)>,
    mut texts: Query<(&CastText, &mut Text)>,
    mut shield: Query<
        &mut Node,
        (
            With<CastShield>,
            Without<CastRoot>,
            Without<CastFill>,
            Without<CastSpark>,
        ),
    >,
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
                icon: flash.icon.clone(),
                time: String::new(),
                shield: false,
                spark: false,
            }),
            _ => None,
        }
    };
    let (player, tgt) = (view(Who::Player), view(Who::Target));
    let pick = |who| if who == Who::Player { &player } else { &tgt };
    for (root, mut node, mut vis) in &mut roots {
        let shown = pick(root.0).is_some();
        node.display = if shown { Display::Flex } else { Display::None };
        vis.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (mut fill, mut node, mut bg) in &mut fills {
        if let Some(v) = pick(fill.who) {
            node.width = percent(v.fill * 100.0);
            if fill.color != v.color {
                fill.color = v.color;
                *bg = glossy(v.color);
            }
        }
    }
    for (CastSpark(who), mut node) in &mut sparks {
        if let Some(v) = pick(*who) {
            node.display = if v.spark && v.fill > 0.0 && v.fill < 1.0 {
                Display::Flex
            } else {
                Display::None
            };
            node.left = percent(v.fill * 100.0);
        }
    }
    for (CastIcon(who), mut img) in &mut icon_nodes {
        if let Some(v) = pick(*who)
            && !v.icon.is_empty()
        {
            let handle = icons.get(&assets, &v.icon);
            if img.image != handle {
                img.image = handle;
            }
        }
    }
    for (CastText(who, field), mut text) in &mut texts {
        if let Some(v) = pick(*who) {
            match field {
                CastField::Name if v.name.is_empty() => set_text(&mut text, ""),
                CastField::Name => set_text(&mut text, &format!("> {}", v.name)),
                CastField::Time => set_text(&mut text, &v.time),
            }
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
            Part::Stacks => set_text(
                &mut text,
                &if a.stacks > 1 {
                    a.stacks.to_string()
                } else {
                    String::new()
                },
            ),
            Part::Icon => {}
        }
    }
}

/// Sim ids restart at 1, so a pending flash could match an unrelated new unit.
fn reset_phase_flash_on_restart(
    mut restarted: MessageReader<crate::game::GameRestarted>,
    mut flash: ResMut<PhaseFlash>,
) {
    if restarted.read().count() > 0 {
        flash.unit = None;
        flash.left = 0.0;
    }
}
