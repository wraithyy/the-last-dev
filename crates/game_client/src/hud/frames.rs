//! Player and target unit frames, cast bars and aura icon rows.

use super::banners::Banner;
use super::{
    ACCENT_WARM, BAR_BG, DIM, HP_GREEN, IconCache, MANA_BLUE, PANEL_BOTTOM, PANEL_EDGE, PANEL_TOP,
    TipSource, fmt_secs, gloss, label, put, set_text, vgrad,
};
use crate::combat::{CombatEvent, SimRead, SimReady, SimUnit, SimView};
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
/// Must-interrupt casts (`SpellDef::kick`): red, and the name says so.
const MUST_KICK: Color = Color::srgb(0.95, 0.12, 0.1);
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
/// The thin burnout strip under the player's mana bar (its node carries the tooltip source).
#[derive(Component)]
struct BurnoutStrip;
#[derive(Component)]
struct BurnoutFill;
#[derive(Component)]
struct BurnoutTrack;
#[derive(Component)]
struct BurnoutLabel;
#[derive(Component)]
struct BurnoutCaption;
const BURNOUT_H: f32 = 28.0;
const BURNOUT_W: f32 = 260.0;
const CAPTION: Color = Color::srgb(0.95, 0.88, 0.7);
const BURNOUT_LOW: Color = Color::srgb(0.85, 0.7, 0.3);
const BURNOUT_HIGH: Color = Color::srgb(0.95, 0.22, 0.18);
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
            .add_systems(
                Update,
                (reset_phase_flash_on_restart, tag_target_frame, recolor_hp),
            )
            .add_systems(
                Update,
                (
                    collect_phase_flash,
                    update_frames,
                    update_burnout_strip,
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

/// HP bars follow the colourblind palette.
fn recolor_hp(
    pal: Res<crate::settings::Palette>,
    mut fills: Query<(&Fill, &mut BackgroundGradient)>,
) {
    if !pal.is_changed() {
        return;
    }
    for (f, mut g) in &mut fills {
        if f.1 == Pool::Hp {
            *g = glossy(pal.health);
        }
    }
}

/// The onboarding points at the target frame (its cast bar sits right under it).
fn tag_target_frame(mut commands: Commands, frames: Query<(Entity, &FrameRoot), Added<FrameRoot>>) {
    for (e, f) in &frames {
        if f.0 == Who::Target {
            commands
                .entity(e)
                .insert(crate::onboarding::Spot::TargetFrame);
        }
    }
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

/// A battery gauge hanging under the player frame (own node: the frame has a fixed height).
/// 260 px wide so it stops short of the target frame's cast bar and aura row.
fn spawn_burnout_strip(commands: &mut Commands, icons: &mut IconCache, assets: &AssetServer) {
    commands
        .spawn((
            BurnoutStrip,
            Button,
            TipSource {
                tag: "burnout",
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                top: px(FRAME_TOP + FRAME_H + 6.0),
                left: px(12),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                width: px(BURNOUT_W),
                ..default()
            },
        ))
        .with_children(|s| {
            s.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(5),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ImageNode::new(icons.get(assets, "battery_low")),
                    Node {
                        width: px(BURNOUT_H),
                        height: px(BURNOUT_H),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                // The battery: the track drains toward red, tick marks split the four tiers.
                row.spawn((
                    BurnoutTrack,
                    Node {
                        height: px(BURNOUT_H),
                        flex_grow: 1.0,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(6)),
                        overflow: Overflow::clip(),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(PANEL_EDGE),
                    BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.85)),
                    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), px(0), px(2), px(0), px(4)),
                ))
                .with_children(|t| {
                    t.spawn((
                        BurnoutFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(0),
                            bottom: px(0),
                            width: percent(0),
                            ..default()
                        },
                        BackgroundColor(BURNOUT_LOW),
                    ));
                    for at in [250.0, 550.0, 850.0] {
                        t.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(at / 10.0),
                                top: px(0),
                                bottom: px(0),
                                width: px(2),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                        ));
                    }
                    t.spawn((
                        BurnoutLabel,
                        label("", 14.0),
                        TextColor::WHITE,
                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    ));
                });
                // Terminal nub of the battery.
                row.spawn((
                    Node {
                        width: px(5),
                        height: px(BURNOUT_H * 0.45),
                        border_radius: BorderRadius::right(px(3)),
                        margin: UiRect::left(px(-5)),
                        ..default()
                    },
                    BackgroundColor(PANEL_EDGE),
                ));
            });
            s.spawn((
                BurnoutCaption,
                label("", 14.0),
                TextColor(CAPTION),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
        });
}

/// Pulse rate (rad/s) of the gauge from Crunch on; Burnt Out beats faster.
const BURNOUT_PULSE: [f32; 4] = [0.0, 0.0, 4.0, 9.0];

/// Fill, colour, label, caption and tooltip source follow the player's burnout; a tier change
/// raises a banner (not on the first read, so joining or loading stays quiet).
#[allow(clippy::too_many_arguments)]
fn update_burnout_strip(
    sim: SimView,
    time: Res<Time>,
    mut last_tier: Local<Option<usize>>,
    mut banners: MessageWriter<Banner>,
    touch: Res<crate::touch::TouchMode>,
    mut place: Query<&mut Node, (With<BurnoutStrip>, Without<BurnoutFill>)>,
    mut strip: Query<&mut TipSource, With<BurnoutStrip>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<BurnoutFill>>,
    mut track: Query<&mut BorderColor, With<BurnoutTrack>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<BurnoutLabel>>,
        Query<&mut Text, With<BurnoutCaption>>,
    )>,
) {
    let b = sim
        .player()
        .and_then(|id| sim.unit(id))
        .map_or(0, |u| u.career.burnout);
    let def = &sim.content().endgame.burnout;
    let tier = def.tier_index(b);
    for mut tip in &mut strip {
        if tip.burnout != Some(b) {
            tip.burnout = Some(b);
        }
    }
    let name = def.tier(b).map_or("", |t| t.name.as_str());
    if let Some(prev) = last_tier.replace(tier)
        && prev != tier
        && let Some(t) = def.tiers.get(tier)
    {
        let up = tier > prev;
        banners.write(Banner {
            title: t.name.to_uppercase(),
            subtitle: if up {
                format!("Burnout rising: {}", def.caption(b))
            } else {
                format!("Burnout easing: {}", def.caption(b))
            },
            color: BURNOUT_LOW.mix(&BURNOUT_HIGH, tier as f32 / 3.0),
        });
    }
    let k = f32::from(b.min(combat_core::BURNOUT_MAX)) / f32::from(combat_core::BURNOUT_MAX);
    // On touch the Kudos wallet sits right under the player frame, so the gauge hangs below it.
    let top = px(FRAME_TOP + FRAME_H + 6.0 + if touch.active { 28.0 } else { 0.0 });
    for mut n in &mut place {
        if n.top != top {
            n.top = top;
        }
    }
    let pulse = BURNOUT_PULSE.get(tier).copied().unwrap_or(0.0);
    let beat = if pulse > 0.0 {
        0.5 + 0.5 * (time.elapsed_secs() * pulse).sin()
    } else {
        0.0
    };
    let base = BURNOUT_LOW.mix(&BURNOUT_HIGH, k);
    for (mut node, mut color) in &mut fill {
        let w = percent(k * 100.0);
        if node.width != w {
            node.width = w;
        }
        put!(color.0, base.mix(&Color::WHITE, beat * 0.35));
    }
    for mut edge in &mut track {
        put!(*edge, BorderColor::all(PANEL_EDGE.mix(&BURNOUT_HIGH, beat)));
    }
    let head = format!("{}  {}%", name.to_uppercase(), b / 10);
    for mut t in &mut texts.p0() {
        set_text(&mut t, &head);
    }
    for mut t in &mut texts.p1() {
        set_text(&mut t, def.caption(b));
    }
}

fn spawn_cast_bar(
    commands: &mut Commands,
    who: Who,
    node: Node,
    icons: &mut IconCache,
    assets: &AssetServer,
) {
    let mut bar = commands.spawn((
        CastRoot(who),
        Node {
            position_type: PositionType::Absolute,
            height: px(CAST_BAR_H),
            display: Display::None,
            ..node
        },
    ));
    // Only the target's bar has a hover tooltip; the player's own cast is already on the action bar.
    if who == Who::Target {
        bar.insert((
            Button,
            TipSource {
                tag: "target-cast",
                ..default()
            },
        ));
    }
    bar.with_children(|root| {
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
                    Button,
                    TipSource {
                        tag: match row {
                            Row::PlayerAuras => "player-aura",
                            Row::TargetDebuffs => "target-aura",
                        },
                        ..default()
                    },
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
    spawn_burnout_strip(&mut commands, &mut icons, &assets);
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
            top: px(super::BELOW_MINIMAP),
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
    sim: &dyn SimRead,
    target: &Target,
    units: &Query<&SimUnit>,
) -> Option<EntityId> {
    match who {
        Who::Player => sim.player(),
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
    sim: SimView,
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
        (
            &BossTag,
            &mut Node,
            &mut Visibility,
            &mut BorderColor,
            &mut BackgroundGradient,
        ),
        (
            Without<FrameRoot>,
            Without<PoolRow>,
            Without<Fill>,
            Without<Disc>,
        ),
    >,
    mut texts: Query<(&TextOf, &mut Text, &mut TextColor, &mut TextFont), Without<BossTagText>>,
    mut tag_texts: Query<(&BossTagText, &mut Text, &mut TextColor), Without<TextOf>>,
    mut discs: Query<(&Disc, &mut BackgroundColor, &mut BorderColor)>,
) {
    let state = |who| tracked(who, &*sim, &target, &units).and_then(|id| sim.unit(id));
    let role = |who| state(who).and_then(|u| crate::classes::role_of_unit(sim.content(), &u.key));
    let boss =
        |who| state(who).is_some_and(|u| u.boss || copy::is_elite(&u.key)) || role(who).is_some();
    for (BossTagText(who), mut text, mut color) in &mut tag_texts {
        let Some(u) = state(*who) else { continue };
        let tag = match (role(*who), u.boss, u.phase) {
            (Some(r), _, _) => r.tag().to_string(),
            (None, true, p) if p > 1 => format!("PROD P{p}"),
            (None, true, _) => "PROD".to_string(),
            (None, false, _) => "ELITE".to_string(),
        };
        set_text(&mut text, &tag);
        put!(
            color.0,
            role(*who).map_or(ACCENT_WARM, crate::classes::role_color)
        );
    }
    for (root, mut node) in &mut roots {
        put!(
            node.display,
            if state(root.0).is_some() {
                Display::Flex
            } else {
                Display::None
            }
        );
    }
    for (tag, mut node, mut vis, mut rim, mut bg) in &mut tags {
        // Players wear their role tag on a neutral plate; bosses keep the red PROD plate.
        let (edge, plate) = match role(tag.0) {
            Some(r) => (
                crate::classes::role_color(r),
                vgrad(Color::srgb(0.12, 0.13, 0.17), Color::srgb(0.05, 0.05, 0.08)),
            ),
            None => (
                ACCENT_WARM,
                vgrad(Color::srgb(0.4, 0.08, 0.06), Color::srgb(0.16, 0.03, 0.03)),
            ),
        };
        put!(*rim, BorderColor::all(edge));
        if *bg != plate {
            *bg = plate;
        }
        let shown = boss(tag.0);
        put!(
            node.display,
            if shown { Display::Flex } else { Display::None }
        );
        vis.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (row, mut node, mut vis) in &mut rows {
        let has_pool = state(row.0).is_some_and(|u| row.1 == Pool::Hp || u.max_mana > 0);
        put!(
            node.display,
            if has_pool {
                Display::Flex
            } else {
                Display::None
            }
        );
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
    for (TextOf(who, field), mut text, mut color, mut font) in &mut texts {
        let Some(u) = state(*who) else { continue };
        match field {
            Field::Name => {
                set_text(&mut text, &u.name);
                // Long names ("Fullstack Engineer") plus a role tag must still fit one line.
                let size = bevy::text::FontSize::Px(if u.name.chars().count() > 14 {
                    13.0
                } else {
                    15.0
                });
                if font.font_size != size {
                    font.font_size = size;
                }
                put!(
                    color.0,
                    if !u.alive {
                        Color::srgb(0.6, 0.6, 0.6)
                    } else if *who == Who::Player {
                        Color::WHITE
                    } else if sim.players().contains(&u.id) {
                        crate::nameplates::PARTY_BLUE
                    } else if u.hostile {
                        Color::srgb(1.0, 0.25, 0.2)
                    } else {
                        Color::srgb(0.35, 1.0, 0.4)
                    }
                );
            }
            // A worn achievement or promotion title (players) replaces the unit's own subtitle.
            Field::Title => set_text(
                &mut text,
                &sim.content()
                    .title_text(&u.title)
                    .or_else(|| {
                        sim.content()
                            .unit_def(&u.key)
                            .map(|d| d.title.as_str())
                            .filter(|t| !t.is_empty())
                    })
                    .map_or_else(String::new, |t| format!("<{t}>")),
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
    sim: SimView,
    mut flash: ResMut<CastFlash>,
    time: Res<Time>,
) {
    flash.left = (flash.left - time.delta_secs()).max(0.0);
    let Some(me) = sim.player() else { return };
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastSucceeded { caster, spell, .. } if *caster == me => {
                if let Some(def) = sim.content().spell(*spell)
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
    let casting = sim.unit(me).and_then(|u| u.cast);
    if let Some(def) = casting.and_then(|c| sim.content().spell(c.spell))
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
    /// The live cast behind the bar, for its tooltip; None for the post-cast flash.
    cast: Option<CastView>,
}

const CAST_LABEL_MAX: usize = 24;

fn cast_view(sim: &dyn SimRead, cast: CastView, remaining_only: bool) -> BarView {
    let def = sim.content().spell(cast.spell);
    let kick = cast.interruptible && def.is_some_and(|d| d.kick);
    let (name, icon) = def.map_or_else(Default::default, |d| {
        let name = if kick {
            format!("KICK: {}", d.name)
        } else {
            d.name.clone()
        };
        (name, d.icon.clone())
    });
    // One line on the narrow bars: "Have You Tried Turning It Off and On Again" would wrap.
    let name = if name.chars().count() > CAST_LABEL_MAX {
        let cut: String = name.chars().take(CAST_LABEL_MAX - 3).collect();
        format!("{}...", cut.trim_end())
    } else {
        name
    };
    let total = cast.total_ms.max(1);
    let secs = |ms: u32| ms as f32 / 1000.0;
    BarView {
        fill: (cast.elapsed_ms as f32 / total as f32).clamp(0.0, 1.0),
        color: if kick {
            MUST_KICK
        } else if cast.interruptible {
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
        cast: Some(cast),
    }
}

fn update_cast_bars(
    sim: SimView,
    target: Res<Target>,
    flash: Res<CastFlash>,
    units: Query<&SimUnit>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut roots: Query<(
        &CastRoot,
        &mut Node,
        &mut Visibility,
        Option<&mut TipSource>,
    )>,
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
        let id = tracked(who, &*sim, &target, &units)?;
        let u = sim.unit(id)?;
        match (u.cast, who) {
            (Some(c), Who::Player) => Some(cast_view(&*sim, c, false)),
            (Some(c), Who::Target) if u.alive => Some(cast_view(&*sim, c, true)),
            (None, Who::Player) if flash.left > 0.0 => flash.color.map(|color| BarView {
                fill: 1.0,
                color,
                name: flash.text.clone(),
                icon: flash.icon.clone(),
                time: String::new(),
                shield: false,
                spark: false,
                cast: None,
            }),
            _ => None,
        }
    };
    let (player, tgt) = (view(Who::Player), view(Who::Target));
    let pick = |who| if who == Who::Player { &player } else { &tgt };
    for (root, mut node, mut vis, tip) in &mut roots {
        let shown = pick(root.0).is_some();
        if let Some(mut tip) = tip {
            tip.cast = pick(root.0).as_ref().and_then(|v| v.cast);
        }
        put!(
            node.display,
            if shown { Display::Flex } else { Display::None }
        );
        vis.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for (mut fill, mut node, mut bg) in &mut fills {
        if let Some(v) = pick(fill.who) {
            put!(node.width, percent(v.fill * 100.0));
            if fill.color != v.color {
                fill.color = v.color;
                *bg = glossy(v.color);
            }
        }
    }
    for (CastSpark(who), mut node) in &mut sparks {
        if let Some(v) = pick(*who) {
            put!(
                node.display,
                if v.spark && v.fill > 0.0 && v.fill < 1.0 {
                    Display::Flex
                } else {
                    Display::None
                }
            );
            put!(node.left, percent(v.fill * 100.0));
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
        put!(
            node.display,
            if tgt.as_ref().is_some_and(|v| v.shield) {
                Display::Flex
            } else {
                Display::None
            }
        );
    }
}

fn update_auras(
    sim: SimView,
    target: Res<Target>,
    units: Query<&SimUnit>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut slots: Query<(&AuraSlot, &mut Node, &mut BorderColor, &mut TipSource)>,
    mut images: Query<(&AuraPart, &mut ImageNode)>,
    mut texts: Query<(&AuraPart, &mut Text)>,
) {
    let auras_of = |row: Row| -> Vec<AuraView> {
        match row {
            Row::PlayerAuras => {
                let Some(u) = sim.unit(player.0) else {
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
                .and_then(|s| sim.unit(s.0))
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
    for (AuraSlot(row, i), mut node, mut border, mut tip) in &mut slots {
        let aura = get(*row, *i);
        tip.aura = aura.cloned();
        put!(
            node.display,
            if aura.is_some() {
                Display::Flex
            } else {
                Display::None
            }
        );
        if let Some(a) = aura {
            put!(
                *border,
                BorderColor::all(if a.harmful {
                    Color::srgb(0.8, 0.15, 0.15)
                } else {
                    Color::srgb(0.3, 0.75, 0.35)
                })
            );
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
