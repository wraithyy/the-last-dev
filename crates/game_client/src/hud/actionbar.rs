//! Action bar: slots are bound to the player's class bar; everything on a slot
//! (icon, tint, sweep, seconds) is derived from the sim's `CooldownView`.

use super::{
    ACCENT, ACCENT_WARM, DIM, DebugHoverSlot, IconCache, KEY_RIM, KEY_SIDE, KEY_TOP, KEY_TOP_HI,
    PANEL_EDGE, asset_key, drop_shadow, fmt_secs, gloss, label, panel_bg, put, set_text, vgrad,
};
use crate::combat::{
    PendingCommands, SimRead, SimReady, SimUnit, SimView, UiNotice, can_act, cast_coffee_break,
    cast_resurrect, cast_slot, toggle_auto_attack,
};
use crate::config::*;
use crate::copy;
use crate::game::GameFlow;
use crate::input::Paused;
use crate::keybinds::{Action, KeyLegend, Keybinds};
use crate::player::Player;
use crate::targeting::Target;
use crate::touch::{self, TouchMode, TouchUi};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use combat_core::{CastError, Content, Effect, PerkView, SpellDef, SpellId, SpellTarget};

const KEY_LABELS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
/// Locked (silenced / school lockout) keycaps: dimmed under a padlock.
const TINT_LOCKED: Color = Color::srgba(0.02, 0.02, 0.05, 0.62);
/// Stunned: every keycap goes grey.
const ICON_STUNNED: Color = Color::srgb(0.35, 0.35, 0.38);
const TINT_MANA: Color = Color::srgba(0.1, 0.2, 0.95, 0.5);
const TINT_RANGE: Color = Color::srgba(0.85, 0.1, 0.1, 0.5);
const SWING_BAR: Color = Color::srgb(0.95, 0.85, 0.5);
const ATTACK_EDGE: Color = Color::srgb(0.75, 0.4, 0.26);
const COFFEE_EDGE: Color = Color::srgb(0.5, 0.34, 0.2);
const COFFEE_GAP: f32 = 10.0;
/// Darkens a pressed key; hover lightens it.
const PRESS: Color = Color::srgba(0.0, 0.0, 0.0, 0.3);
const HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);
const BAR_PAD: f32 = 8.0;
const BAR_BOTTOM: f32 = 20.0;
/// Keycap height: slightly taller than wide so the lip shows below the face.
const KEY_H: f32 = ACTION_SLOT + 4.0;
const FACE_INSET: f32 = 2.0;
const LIP: f32 = 6.0;
/// How far a pressed key's face sinks, like switch travel.
const TRAVEL: f32 = 3.0;
const ATTACK_PLATE: f32 = 64.0;
const ATTACK_H: f32 = 64.0;
const ATTACK_BTN: f32 = 38.0;
const ATTACK_LIP: f32 = 7.0;
/// Clearance between the right brace and the attack control.
const ATTACK_GAP: f32 = 30.0;
const TOOLTIP_W: f32 = 310.0;
const TOOLTIP_BOTTOM: f32 = 108.0;

/// Radial clock-wipe overlay; see `assets/shaders/cooldown_sweep.wgsl`.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct CooldownMaterial {
    /// x = fraction of the cooldown still remaining. A Vec4 to keep the uniform 16-byte aligned.
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for CooldownMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/cooldown_sweep.wgsl".into()
    }
}

#[derive(Component)]
struct Slot(usize);
/// The bar's frame, re-centred when the touch layout shows fewer slots.
#[derive(Component)]
struct BarRoot;
/// Touch only: flips the bar between slots 1-6 and 7-0.
#[derive(Component)]
struct PageToggle;
#[derive(Component)]
struct PageLabel;
/// Touch bar page: false shows the first `TOUCH_PAGE` slots, true the rest.
#[derive(Resource, Default)]
struct BarPage(bool);
/// Slots on the first touch page; the toggle makes the other four reachable.
const TOUCH_PAGE: usize = 6;
/// An accent key's desktop anchor (from the screen centre and bottom); the touch layout moves
/// the key into the thumb cluster and restores this when it ends.
#[derive(Component)]
struct AccentRoot {
    action: Action,
    left: f32,
    bottom: f32,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Icon,
    Tint,
    Sweep,
    Seconds,
    Press,
    /// Padlock shown on locked and unlearned slots.
    Lock,
    /// "Lv N" on a slot whose spell is not learned yet.
    LearnAt,
}
#[derive(Component)]
struct SlotPart(usize, Part);
/// Sinking top face of a spell keycap.
#[derive(Component)]
struct KeyFace(usize);
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct TipLine(usize);
#[derive(Component)]
struct TipName;
#[derive(Component)]
struct TipSig;
/// The Attack (auto attack) button, left of slot 1.
#[derive(Component)]
struct AttackButton;
#[derive(Component)]
struct AttackIcon;
#[derive(Component)]
struct AttackSwing;
#[derive(Component)]
struct AttackRing;
/// Hover / press overlay on the attack button.
#[derive(Component)]
struct AttackGlow;

/// The Coffee Break button, right of the attack button.
#[derive(Component)]
struct CoffeeButton;
#[derive(Component)]
struct CoffeeIcon;
/// Channel progress while sipping, cooldown remainder afterwards.
#[derive(Component)]
struct CoffeeGauge;
#[derive(Component)]
struct CoffeeRing;
#[derive(Component)]
struct CoffeeGlow;

/// The co-op resurrect button, right of coffee; only shown while others are in the group.
#[derive(Component)]
struct RezRoot;
#[derive(Component)]
struct RezButton;
#[derive(Component)]
struct RezIcon;
/// Cast progress of the reboot.
#[derive(Component)]
struct RezGauge;
#[derive(Component)]
struct RezRing;
#[derive(Component)]
struct RezGlow;

/// The movement keycaps stacked above attack and coffee: sprint (false) and the e-scooter (true).
#[derive(Component)]
struct ExtraRing(bool);
#[derive(Component)]
struct ExtraButton(bool);
#[derive(Component)]
struct ExtraIcon(bool);
#[derive(Component)]
struct ExtraGlow(bool);
/// Cooldown remainder (sprint), mount cast progress, or full while riding.
#[derive(Component)]
struct ExtraGauge(bool);

pub struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BarPage>()
            .add_systems(Startup, spawn_bar)
            .add_systems(
                Update,
                (page_press, touch_bar, touch_accents, tag_spots).chain(),
            )
            .add_systems(
                Update,
                (
                    click_slots,
                    click_attack,
                    click_coffee,
                    click_rez,
                    click_extras,
                    update_extras,
                    update_slots,
                    update_attack,
                    update_coffee,
                    update_rez,
                    update_tooltip,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// Onboarding highlight anchors on the keys it points at.
fn tag_spots(
    mut commands: Commands,
    slots: Query<(Entity, &Slot), Added<Slot>>,
    attack: Query<Entity, Added<AttackButton>>,
    extras: Query<(Entity, &ExtraButton), Added<ExtraButton>>,
) {
    use crate::onboarding::Spot;
    for (e, Slot(i)) in &slots {
        let i = u8::try_from(*i).unwrap_or(u8::MAX);
        commands.entity(e).insert(Spot::Slot(i));
    }
    for e in &attack {
        commands.entity(e).insert(Spot::AutoAttack);
    }
    for (e, ExtraButton(mount)) in &extras {
        if *mount {
            commands.entity(e).insert(Spot::Mount);
        }
    }
}

fn bar_width() -> f32 {
    let n = KEY_LABELS.len() as f32;
    n * ACTION_SLOT + (n - 1.0) * ACTION_GAP + 2.0 * BAR_PAD + 4.0
}

fn bar_height() -> f32 {
    KEY_H + 2.0 * BAR_PAD + 4.0
}

/// Legend printed on a keycap corner; a dark pill keeps it readable over any icon art.
fn legend(action: Action, key: &str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            top: px(2),
            left: px(2),
            padding: UiRect::axes(px(3), px(0)),
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        children![(
            KeyLegend(action),
            label(key, 11.0),
            TextColor(Color::srgb(0.9, 0.95, 1.0))
        )],
    )
}

/// Face rectangle of a keycap at rest; pressing shifts it down by `travel` and eats the lip.
fn face_node(inset: f32, lip: f32, travel: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(inset),
        right: px(inset),
        top: px(inset + travel),
        bottom: px(lip - travel),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(6)),
        ..default()
    }
}

/// Padlock drawn from two nodes (shackle ring + body), hidden until a slot is locked.
fn padlock() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: percent(50),
            top: percent(50),
            width: px(16),
            height: px(18),
            margin: UiRect::new(px(-8), px(0), px(-11), px(0)),
            display: Display::None,
            ..default()
        },
        children![
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(3),
                    top: px(0),
                    width: px(10),
                    height: px(11),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::top(px(5)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.85, 0.88, 0.95)),
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    bottom: px(0),
                    width: px(16),
                    height: px(10),
                    border_radius: BorderRadius::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.85, 0.88, 0.95)),
            ),
        ],
    )
}

fn spawn_bar(mut commands: Commands, mut materials: ResMut<Assets<CooldownMaterial>>) {
    let width = bar_width();
    let height = bar_height();
    commands
        .spawn((
            BarRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(BAR_BOTTOM),
                left: percent(50),
                margin: UiRect::left(px(-width / 2.0)),
                padding: UiRect::all(px(BAR_PAD)),
                column_gap: px(ACTION_GAP),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(9)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            panel_bg(),
            drop_shadow(),
        ))
        .with_children(|bar| {
            // Curly braces flank the keys, so the whole bar reads as a code block.
            for (glyph, left) in [("{", true), ("}", false)] {
                let side = px(-28);
                bar.spawn((
                    label(glyph, 38.0),
                    TextColor(ACCENT.with_alpha(0.6)),
                    Node {
                        position_type: PositionType::Absolute,
                        top: px((height - 4.0 - 50.0) / 2.0),
                        left: if left { side } else { Val::Auto },
                        right: if left { Val::Auto } else { side },
                        ..default()
                    },
                ));
            }
            // Status LEDs sit on the case's top edge like a keyboard's lock indicators.
            for (i, c) in [
                Color::srgb(0.3, 0.9, 0.4),
                Color::srgb(1.0, 0.75, 0.2),
                Color::srgb(0.3, 0.33, 0.4),
            ]
            .into_iter()
            .enumerate()
            {
                bar.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(-4),
                        right: px(12.0 + i as f32 * 9.0),
                        width: px(5),
                        height: px(5),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(c),
                ));
            }
            for (i, key) in KEY_LABELS.iter().enumerate() {
                bar.spawn((
                    Slot(i),
                    Button,
                    Node {
                        width: px(ACTION_SLOT),
                        height: px(KEY_H),
                        border_radius: BorderRadius::all(px(8)),
                        ..default()
                    },
                    vgrad(Color::srgb(0.1, 0.11, 0.15), KEY_SIDE),
                ))
                .with_children(|slot| {
                    slot.spawn((
                        KeyFace(i),
                        face_node(FACE_INSET, LIP, 0.0),
                        BorderColor::all(KEY_RIM),
                        vgrad(KEY_TOP_HI, KEY_TOP),
                    ))
                    .with_children(|face| {
                        let fill = Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(5)),
                            ..default()
                        };
                        face.spawn((SlotPart(i, Part::Icon), ImageNode::default(), fill.clone()));
                        face.spawn((
                            SlotPart(i, Part::Tint),
                            fill.clone(),
                            BackgroundColor(Color::NONE),
                        ));
                        face.spawn((
                            SlotPart(i, Part::Sweep),
                            MaterialNode(materials.add(CooldownMaterial { params: Vec4::ZERO })),
                            Node {
                                display: Display::None,
                                ..fill.clone()
                            },
                        ));
                        face.spawn(gloss(0.14));
                        face.spawn((
                            SlotPart(i, Part::Seconds),
                            label("", 18.0),
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                top: px(9),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            TextLayout::justify(Justify::Center),
                            TextColor(Color::srgb(1.0, 0.95, 0.7)),
                        ));
                        face.spawn((SlotPart(i, Part::Press), fill, BackgroundColor(Color::NONE)));
                        face.spawn((SlotPart(i, Part::Lock), padlock()));
                        face.spawn((
                            SlotPart(i, Part::LearnAt),
                            label("", 10.0),
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                bottom: px(3),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            TextLayout::justify(Justify::Center),
                            TextColor(DIM),
                        ));
                        face.spawn(legend(Action::Slot(i as u8), key));
                    });
                });
            }
            bar.spawn((
                PageToggle,
                Button,
                Node {
                    width: px(touch::SLOT_W),
                    height: px(touch::SLOT_H),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(8)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                BackgroundColor(KEY_TOP),
                children![(PageLabel, label("", 16.0), TextColor(ACCENT))],
            ));
        });

    spawn_accents(&mut commands);

    commands
        .spawn((
            Tooltip,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(TOOLTIP_BOTTOM),
                left: percent(50),
                margin: UiRect::left(px(-TOOLTIP_W / 2.0)),
                width: px(TOOLTIP_W),
                padding: UiRect::axes(px(10), px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(20),
        ))
        .with_children(|t| {
            // Syntax-highlighted signature: keyword, function name, rest of the signature.
            let span = |size: f32, color: Color| {
                (
                    TextSpan::new(""),
                    TextFont::from_font_size(size),
                    TextColor(color),
                )
            };
            t.spawn((
                label("fn ", 14.0),
                TextColor(Color::srgb(0.78, 0.55, 0.95)),
                children![
                    (TipName, span(14.0, Color::srgb(0.95, 0.85, 0.55))),
                    (TipSig, span(14.0, DIM)),
                ],
            ));
            t.spawn((
                TipLine(0),
                label("", 12.0),
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
            ));
            t.spawn((
                TipLine(1),
                label("", 12.0),
                TextColor(Color::srgb(0.5, 0.75, 0.55)),
            ));
            // Career Ladder talents that change this spell.
            t.spawn((TipLine(2), label("", 12.0), TextColor(ACCENT_WARM)));
        });
}

/// Look of one accent keycap: plate behind, key face, face edge and the thin gauge under it.
struct Accent {
    action: Action,
    legend: &'static str,
    plate: (Color, Color),
    face: (Color, Color),
    edge: Color,
    gauge: (Color, Color),
}

const ATTACK_ACCENT: Accent = Accent {
    action: Action::AutoAttack,
    legend: "T",
    plate: (Color::srgb(0.2, 0.08, 0.05), Color::srgb(0.06, 0.03, 0.03)),
    face: (Color::srgb(0.92, 0.45, 0.24), Color::srgb(0.58, 0.2, 0.12)),
    edge: ATTACK_EDGE,
    gauge: (SWING_BAR, Color::srgb(0.6, 0.45, 0.15)),
};

const REZ_ACCENT: Accent = Accent {
    action: Action::Resurrect,
    legend: "V",
    plate: (Color::srgb(0.05, 0.1, 0.12), Color::srgb(0.02, 0.04, 0.05)),
    face: (Color::srgb(0.3, 0.62, 0.66), Color::srgb(0.12, 0.3, 0.34)),
    edge: REZ_EDGE,
    gauge: (Color::srgb(0.55, 0.95, 0.7), Color::srgb(0.2, 0.6, 0.4)),
};
const REZ_EDGE: Color = Color::srgb(0.2, 0.42, 0.45);

const SPRINT_ACCENT: Accent = Accent {
    action: Action::Sprint,
    legend: "X",
    plate: (Color::srgb(0.05, 0.1, 0.06), Color::srgb(0.02, 0.04, 0.03)),
    face: (Color::srgb(0.35, 0.7, 0.4), Color::srgb(0.14, 0.34, 0.18)),
    edge: EXTRA_EDGE,
    gauge: (Color::srgb(0.6, 1.0, 0.6), Color::srgb(0.2, 0.6, 0.3)),
};
const SCOOTER_ACCENT: Accent = Accent {
    action: Action::Scooter,
    legend: "Y",
    plate: (Color::srgb(0.12, 0.07, 0.03), Color::srgb(0.05, 0.03, 0.02)),
    face: (Color::srgb(0.95, 0.55, 0.2), Color::srgb(0.5, 0.25, 0.08)),
    edge: EXTRA_EDGE,
    gauge: (Color::srgb(1.0, 0.75, 0.35), Color::srgb(0.7, 0.4, 0.1)),
};
const EXTRA_EDGE: Color = Color::srgb(0.4, 0.4, 0.3);

const COFFEE_ACCENT: Accent = Accent {
    action: Action::Coffee,
    legend: "C",
    plate: (Color::srgb(0.12, 0.08, 0.05), Color::srgb(0.04, 0.03, 0.03)),
    face: (Color::srgb(0.62, 0.42, 0.26), Color::srgb(0.3, 0.17, 0.1)),
    edge: COFFEE_EDGE,
    gauge: (Color::srgb(0.45, 0.68, 1.0), Color::srgb(0.2, 0.35, 0.8)),
};

/// Wider accent keycaps kept apart from the spell row, like an Esc key: auto attack, then coffee.
/// (left offset from screen centre, bottom) for the gadget bar: stacked on top of the
/// sprint/scooter keys, clear of the meter and combat log column on the right edge.
pub fn gadget_anchor() -> (f32, f32) {
    (
        bar_width() / 2.0 + ATTACK_GAP,
        BAR_BOTTOM + 2.0 * bar_height() + 6.0 + 8.0,
    )
}

fn spawn_accents(commands: &mut Commands) {
    // Right of the bar like an Enter key; the left side belongs to the combat log.
    let left = bar_width() / 2.0 + ATTACK_GAP;
    let above = BAR_BOTTOM + bar_height() + 6.0;
    for (k, look, mount) in [(0.0, &SPRINT_ACCENT, false), (1.0, &SCOOTER_ACCENT, true)] {
        accent_key(
            commands,
            left + k * (ATTACK_PLATE + COFFEE_GAP),
            above,
            look,
            (
                ExtraRing(mount),
                ExtraButton(mount),
                ExtraIcon(mount),
                ExtraGlow(mount),
                ExtraGauge(mount),
            ),
        );
    }
    accent_key(
        commands,
        left,
        BAR_BOTTOM,
        &ATTACK_ACCENT,
        (
            AttackRing,
            AttackButton,
            AttackIcon,
            AttackGlow,
            AttackSwing,
        ),
    );
    accent_key(
        commands,
        left + ATTACK_PLATE + COFFEE_GAP,
        BAR_BOTTOM,
        &COFFEE_ACCENT,
        (
            CoffeeRing,
            CoffeeButton,
            CoffeeIcon,
            CoffeeGlow,
            CoffeeGauge,
        ),
    );
    let rez = accent_key(
        commands,
        left + 2.0 * (ATTACK_PLATE + COFFEE_GAP),
        BAR_BOTTOM,
        &REZ_ACCENT,
        (RezRing, RezButton, RezIcon, RezGlow, RezGauge),
    );
    commands.entity(rez).insert(RezRoot);
}

/// `markers` = (ring, button, icon, glow, gauge fill) components of the control being built.
fn accent_key<R: Bundle, B: Bundle, I: Bundle, G: Bundle, F: Bundle>(
    commands: &mut Commands,
    left: f32,
    bottom: f32,
    look: &Accent,
    markers: (R, B, I, G, F),
) -> Entity {
    let (ring_m, button_m, icon_m, glow_m, gauge_m) = markers;
    commands
        .spawn((
            AccentRoot {
                action: look.action,
                left,
                bottom,
            },
            Node {
                position_type: PositionType::Absolute,
                bottom: px(bottom),
                left: percent(50),
                margin: UiRect::left(px(left)),
                width: px(ATTACK_PLATE),
                height: px(bar_height()),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(4),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                ring_m,
                Node {
                    width: px(ATTACK_PLATE),
                    height: px(ATTACK_H),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                vgrad(look.plate.0, look.plate.1),
                drop_shadow(),
            ))
            .with_children(|ring| {
                ring.spawn((
                    button_m,
                    Button,
                    Node {
                        overflow: Overflow::clip(),
                        ..face_node(1.0, ATTACK_LIP, 0.0)
                    },
                    BorderColor::all(look.edge),
                    vgrad(look.face.0, look.face.1),
                ))
                .with_children(|b| {
                    b.spawn((
                        icon_m,
                        ImageNode::default(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(9),
                            top: px(5),
                            width: px(ATTACK_BTN),
                            height: px(ATTACK_BTN),
                            border_radius: BorderRadius::all(px(5)),
                            ..default()
                        },
                    ));
                    b.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(50),
                            ..default()
                        },
                        vgrad(
                            Color::srgba(1.0, 1.0, 1.0, 0.16),
                            Color::srgba(1.0, 1.0, 1.0, 0.0),
                        ),
                    ));
                    b.spawn((
                        glow_m,
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ));
                    b.spawn(legend(look.action, look.legend));
                });
            });
            root.spawn((
                Node {
                    width: px(ATTACK_PLATE - 12.0),
                    height: px(6),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                BackgroundColor(Color::srgb(0.03, 0.03, 0.05)),
                children![(
                    gauge_m,
                    Node {
                        height: percent(100),
                        width: percent(0),
                        ..default()
                    },
                    vgrad(look.gauge.0, look.gauge.1),
                )],
            ));
        })
        .id()
}

fn page_press(
    toggle: Single<&Interaction, (With<PageToggle>, Changed<Interaction>)>,
    mut page: ResMut<BarPage>,
) {
    if **toggle == Interaction::Pressed {
        page.0 = !page.0;
    }
}

/// Touch: bigger keycaps, only one page of them, and a toggle for the rest. Every slot stays
/// reachable (and its number key keeps working); desktop gets its original geometry back.
fn touch_bar(
    touch: Res<TouchMode>,
    page: Res<BarPage>,
    mut bar: Single<&mut Node, (With<BarRoot>, Without<Slot>, Without<PageToggle>)>,
    mut toggle: Single<&mut Node, (With<PageToggle>, Without<Slot>, Without<BarRoot>)>,
    mut label_text: Single<&mut Text, With<PageLabel>>,
    mut slots: Query<(&Slot, &mut Node), (Without<BarRoot>, Without<PageToggle>)>,
) {
    if !touch.is_changed() && !page.is_changed() {
        return;
    }
    let (first, end) = if page.0 {
        (TOUCH_PAGE, KEY_LABELS.len())
    } else {
        (0, TOUCH_PAGE)
    };
    for (Slot(i), mut node) in &mut slots {
        let (show, w, h) = if touch.active {
            ((first..end).contains(i), touch::SLOT_W, touch::SLOT_H)
        } else {
            (true, ACTION_SLOT, KEY_H)
        };
        node.display = if show { Display::Flex } else { Display::None };
        node.width = px(w);
        node.height = px(h);
    }
    toggle.display = if touch.active {
        Display::Flex
    } else {
        Display::None
    };
    set_text(&mut label_text, if page.0 { "1-6" } else { "7-0" });
    let width = if touch.active {
        let n = (end - first + 1) as f32;
        n * touch::SLOT_W + (n - 1.0) * ACTION_GAP + 2.0 * BAR_PAD + 4.0
    } else {
        bar_width()
    };
    bar.margin = UiRect::left(px(-width / 2.0));
}

/// Touch: the accent keys are the thumb cluster. Auto attack becomes the big primary key, coffee
/// sits in the arc, sprint and ride wait in the "more" tray, revive goes beside the bar.
fn touch_accents(
    touch: Res<TouchMode>,
    ui: Res<TouchUi>,
    mut roots: Query<(&AccentRoot, &mut Node, &mut UiTransform)>,
) {
    if !touch.is_changed() && !ui.is_changed() {
        return;
    }
    for (root, mut node, mut transform) in &mut roots {
        if !touch.active {
            node.left = percent(50);
            node.right = Val::Auto;
            node.margin = UiRect::left(px(root.left));
            node.bottom = px(root.bottom);
            if node.display == Display::None {
                node.display = Display::Flex;
            }
            transform.scale = Vec2::ONE;
            continue;
        }
        // Centre of the keycap plate; it sits 5 px above the root's centre (plate + gauge overflow).
        let (centre, scale, from_left) = match root.action {
            Action::AutoAttack => (touch::primary(), touch::PRIMARY_SCALE, false),
            Action::Coffee => (touch::arc(180.0), 1.0, false),
            Action::Sprint => (touch::tray(0), 1.0, false),
            Action::Scooter => (touch::tray(1), 1.0, false),
            _ => (Vec2::new(REZ_TOUCH.x, REZ_TOUCH.y), 1.0, true),
        };
        let side = px(centre.x - ATTACK_PLATE / 2.0);
        (node.left, node.right) = if from_left {
            (side, Val::Auto)
        } else {
            (Val::Auto, side)
        };
        node.margin = UiRect::ZERO;
        node.bottom = px(centre.y - bar_height() / 2.0 - 5.0);
        transform.scale = Vec2::splat(scale);
        // Revive manages its own visibility (group only).
        if matches!(root.action, Action::Sprint | Action::Scooter) {
            node.display = if ui.more {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

/// Revive key on touch: left of the action bar, clear of the stick ring (centre from bottom-left).
const REZ_TOUCH: Vec2 = Vec2::new(236.0, 60.0);

fn click_slots(
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    slots: Query<(&Slot, &Interaction), Changed<Interaction>>,
) {
    if paused.0 || !can_act(&*sim, &flow, player.0) {
        return;
    }
    for (Slot(i), interaction) in &slots {
        if *interaction == Interaction::Pressed {
            cast_slot(&*sim, &mut pending, player.0, *i);
        }
    }
}

fn click_attack(
    paused: Res<Paused>,
    sim: SimView,
    target: Res<Target>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    player: Single<&SimUnit, With<Player>>,
    button: Single<&Interaction, (With<AttackButton>, Changed<Interaction>)>,
) {
    if !paused.0 && **button == Interaction::Pressed {
        toggle_auto_attack(
            &*sim,
            &mut pending,
            &mut notices,
            player.0,
            target.0.is_some(),
        );
    }
}

/// The keycap rim flashes while auto attack is on; the thin bar shows the swing timer.
fn update_attack(
    time: Res<Time>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut button: Single<
        (&Interaction, &mut BorderColor, &mut Node),
        (With<AttackButton>, Without<AttackSwing>),
    >,
    mut ring: Single<&mut BorderColor, (With<AttackRing>, Without<AttackButton>)>,
    mut icon: Single<&mut ImageNode, With<AttackIcon>>,
    mut glow: Single<&mut BackgroundColor, With<AttackGlow>>,
    mut swing: Single<&mut Node, (With<AttackSwing>, Without<AttackButton>)>,
) {
    let attack_icon = sim
        .unit(player.0)
        .and_then(|u| sim.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or("nitpick", |a| a.icon.as_str());
    let handle = icons.get(&assets, attack_icon);
    if icon.image != handle {
        icon.image = handle;
    }
    let state = sim.unit(player.0);
    let on = state.is_some_and(|u| u.auto_attack);
    let k = 0.5 + 0.5 * (time.elapsed_secs() * 7.0).sin();
    let (interaction, border, node) = &mut *button;
    **border = BorderColor::all(if on {
        ATTACK_EDGE.mix(&Color::WHITE, 0.8 * k)
    } else {
        ATTACK_EDGE
    });
    **ring = BorderColor::all(if on {
        PANEL_EDGE.mix(&ACCENT_WARM, k)
    } else {
        PANEL_EDGE
    });
    let pressed = **interaction == Interaction::Pressed;
    let (top, bottom) = key_travel(1.0, ATTACK_LIP, pressed);
    if node.top != top {
        node.top = top;
        node.bottom = bottom;
    }
    glow.0 = match **interaction {
        Interaction::Pressed => PRESS,
        Interaction::Hovered => HOVER,
        Interaction::None => Color::NONE,
    };
    let swing_ms = state
        .and_then(|u| sim.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or(0, |a| a.swing_ms);
    put!(
        swing.width,
        match state {
            Some(u) if on && swing_ms > 0 && u.swing_remaining_ms > 0 => {
                let done = 1.0 - (u.swing_remaining_ms as f32 / swing_ms as f32).clamp(0.0, 1.0);
                percent(100.0 * done)
            }
            _ => percent(0),
        }
    );
}

fn click_coffee(
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    button: Single<&Interaction, (With<CoffeeButton>, Changed<Interaction>)>,
) {
    if !paused.0 && **button == Interaction::Pressed && can_act(&*sim, &flow, player.0) {
        cast_coffee_break(&*sim, &mut pending, player.0);
    }
}

/// The rim glows while sipping; the icon greys out while the break cannot be taken.
fn update_coffee(
    time: Res<Time>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut button: Single<
        (&Interaction, &mut BorderColor, &mut Node),
        (With<CoffeeButton>, Without<CoffeeGauge>),
    >,
    mut ring: Single<&mut BorderColor, (With<CoffeeRing>, Without<CoffeeButton>)>,
    mut icon: Single<&mut ImageNode, With<CoffeeIcon>>,
    mut glow: Single<&mut BackgroundColor, With<CoffeeGlow>>,
    mut gauge: Single<&mut Node, (With<CoffeeGauge>, Without<CoffeeButton>)>,
) {
    let Some(spell) = sim.content().channel_spell() else {
        return;
    };
    let handle = icons.get(&assets, &spell.icon);
    if icon.image != handle {
        icon.image = handle;
    }
    let view = sim.cooldown(player.0, spell.id);
    let channel = sim
        .unit(player.0)
        .and_then(|u| u.cast)
        .filter(|c| c.spell == spell.id);
    let k = 0.5 + 0.5 * (time.elapsed_secs() * 7.0).sin();
    let (interaction, border, node) = &mut *button;
    **border = BorderColor::all(if channel.is_some() {
        COFFEE_EDGE.mix(&Color::WHITE, 0.8 * k)
    } else {
        COFFEE_EDGE
    });
    **ring = BorderColor::all(if channel.is_some() {
        PANEL_EDGE.mix(&COFFEE_ACCENT.gauge.0, k)
    } else {
        PANEL_EDGE
    });
    icon.color = if channel.is_some() || view.usable.is_ok() {
        Color::WHITE
    } else {
        ICON_STUNNED
    };
    let pressed = **interaction == Interaction::Pressed;
    let (top, bottom) = key_travel(1.0, ATTACK_LIP, pressed);
    if node.top != top {
        node.top = top;
        node.bottom = bottom;
    }
    glow.0 = match **interaction {
        Interaction::Pressed => PRESS,
        Interaction::Hovered => HOVER,
        Interaction::None => Color::NONE,
    };
    put!(
        gauge.width,
        match channel {
            Some(c) => {
                percent(
                    100.0 * (1.0 - c.elapsed_ms as f32 / c.total_ms.max(1) as f32).clamp(0.0, 1.0),
                )
            }
            None if view.cd_total_ms > 0 => {
                percent(
                    100.0 * (view.cd_remaining_ms as f32 / view.cd_total_ms as f32).clamp(0.0, 1.0),
                )
            }
            None => percent(0),
        }
    );
}

fn click_extras(
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    buttons: Query<(&Interaction, &ExtraButton), Changed<Interaction>>,
) {
    if paused.0 || !can_act(&*sim, &flow, player.0) {
        return;
    }
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            crate::office::cast_world_spell(&*sim, &mut pending, player.0, b.0);
        }
    }
}

/// Lit while the ride or sprint is on; greyed while it cannot be used (indoors, in combat,
/// cooling down). The gauge shows the mount cast, the ride, or the sprint cooldown.
#[allow(clippy::type_complexity)] // Bevy query filter tuples
fn update_extras(
    time: Res<Time>,
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut buttons: Query<
        (&ExtraButton, &Interaction, &mut BorderColor, &mut Node),
        Without<ExtraGauge>,
    >,
    mut rings: Query<(&ExtraRing, &mut BorderColor), Without<ExtraButton>>,
    mut icons_q: Query<(&ExtraIcon, &mut ImageNode)>,
    mut glows: Query<(&ExtraGlow, &mut BackgroundColor)>,
    mut gauges: Query<(&ExtraGauge, &mut Node), Without<ExtraButton>>,
) {
    let content = sim.content();
    let me = sim.unit(player.0);
    let k = 0.5 + 0.5 * (time.elapsed_secs() * 7.0).sin();
    let state = |mount: bool| {
        let spell = if mount {
            content.mount_spell()
        } else {
            content.sprint_spell()
        }?;
        let aura = match &spell.effect {
            Effect::Aura { aura, .. } => aura.as_str(),
            _ => "",
        };
        let on = me.is_some_and(|u| u.auras.iter().any(|a| a.key == aura));
        let casting = me.and_then(|u| u.cast).filter(|c| c.spell == spell.id);
        Some((spell, on, casting, sim.cooldown(player.0, spell.id)))
    };
    for (b, interaction, mut border, mut node) in &mut buttons {
        let Some((_, on, ..)) = state(b.0) else {
            continue;
        };
        let look = if b.0 { &SCOOTER_ACCENT } else { &SPRINT_ACCENT };
        *border = BorderColor::all(if on {
            look.edge.mix(&Color::WHITE, 0.8 * k)
        } else {
            look.edge
        });
        let (top, bottom) = key_travel(1.0, ATTACK_LIP, *interaction == Interaction::Pressed);
        if node.top != top {
            node.top = top;
            node.bottom = bottom;
        }
    }
    for (r, mut border) in &mut rings {
        let Some((_, on, ..)) = state(r.0) else {
            continue;
        };
        let look = if r.0 { &SCOOTER_ACCENT } else { &SPRINT_ACCENT };
        *border = BorderColor::all(if on {
            PANEL_EDGE.mix(&look.gauge.0, k)
        } else {
            PANEL_EDGE
        });
    }
    for (i, mut img) in &mut icons_q {
        let Some((spell, on, casting, cd)) = state(i.0) else {
            continue;
        };
        let handle = icons.get(&assets, &spell.icon);
        if img.image != handle {
            img.image = handle;
        }
        // The mount key also gets you off, so a rider's key stays lit.
        img.color = if on || casting.is_some() || cd.usable.is_ok() {
            Color::WHITE
        } else {
            ICON_STUNNED
        };
    }
    for (g, mut bg) in &mut glows {
        let Some(interaction) = buttons
            .iter()
            .find(|(b, ..)| b.0 == g.0)
            .map(|(_, i, ..)| *i)
        else {
            continue;
        };
        bg.0 = match interaction {
            Interaction::Pressed => PRESS,
            Interaction::Hovered => HOVER,
            Interaction::None => Color::NONE,
        };
    }
    for (g, mut node) in &mut gauges {
        let Some((_, on, casting, cd)) = state(g.0) else {
            continue;
        };
        let frac = match casting {
            Some(c) => c.elapsed_ms as f32 / c.total_ms.max(1) as f32,
            None if g.0 && on => 1.0,
            None if cd.cd_total_ms > 0 => cd.cd_remaining_ms as f32 / cd.cd_total_ms as f32,
            None => 0.0,
        };
        put!(node.width, percent(100.0 * frac.clamp(0.0, 1.0)));
    }
}

fn click_rez(
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    button: Single<&Interaction, (With<RezButton>, Changed<Interaction>)>,
) {
    if !paused.0 && **button == Interaction::Pressed && can_act(&*sim, &flow, player.0) {
        cast_resurrect(&*sim, &mut pending, player.0);
    }
}

/// Shown only with company; lit while a teammate lies dead, the gauge fills with the cast.
fn update_rez(
    sim: SimView,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut root: Single<&mut Node, (With<RezRoot>, Without<RezButton>, Without<RezGauge>)>,
    mut button: Single<
        (&Interaction, &mut Node),
        (With<RezButton>, Without<RezGauge>, Without<RezRoot>),
    >,
    mut icon: Single<&mut ImageNode, With<RezIcon>>,
    mut glow: Single<&mut BackgroundColor, With<RezGlow>>,
    mut gauge: Single<&mut Node, (With<RezGauge>, Without<RezButton>, Without<RezRoot>)>,
) {
    let Some(spell) = sim.resurrect_of(player.0) else {
        return;
    };
    let me = player.0;
    let grouped = sim.players().len() > 1;
    let want = if grouped {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != want {
        root.display = want;
    }
    if !grouped {
        return;
    }
    let handle = icons.get(&assets, &spell.icon);
    if icon.image != handle {
        icon.image = handle;
    }
    let someone_down = sim
        .players()
        .iter()
        .any(|p| *p != me && sim.unit(*p).is_some_and(|u| !u.alive));
    let ready = sim.unit(me).is_some_and(|u| u.alive && !u.in_combat);
    icon.color = if someone_down && ready {
        Color::WHITE
    } else {
        ICON_STUNNED
    };
    let (interaction, node) = &mut *button;
    let (top, bottom) = key_travel(1.0, ATTACK_LIP, **interaction == Interaction::Pressed);
    if node.top != top {
        node.top = top;
        node.bottom = bottom;
    }
    glow.0 = match **interaction {
        Interaction::Pressed => PRESS,
        Interaction::Hovered => HOVER,
        Interaction::None => Color::NONE,
    };
    let cast = sim
        .unit(me)
        .and_then(|u| u.cast)
        .filter(|c| c.spell == spell.id);
    put!(
        gauge.width,
        percent(cast.map_or(0.0, |c| {
            100.0 * (c.elapsed_ms as f32 / c.total_ms.max(1) as f32).clamp(0.0, 1.0)
        }))
    );
}

/// Top and bottom offsets of a keycap face; pressing sinks it by `TRAVEL`.
fn key_travel(inset: f32, lip: f32, pressed: bool) -> (Val, Val) {
    let t = if pressed { TRAVEL } else { 0.0 };
    (px(inset + t), px(lip - t))
}

/// Per-slot verdict derived from the sim each frame.
#[derive(Clone, Copy, Default)]
struct SlotView {
    spell: Option<SpellId>,
    learned: bool,
    /// Silenced or school-locked: dim under a padlock.
    locked: bool,
    usable: Option<CastError>,
}

fn update_slots(
    sim: SimView,
    flow: Res<GameFlow>,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut materials: ResMut<Assets<CooldownMaterial>>,
    player: Single<&SimUnit, With<Player>>,
    slots: Query<(&Slot, &Interaction)>,
    mut faces: Query<(&KeyFace, &mut Node, &mut BorderColor), Without<SlotPart>>,
    mut nodes: Query<(
        &SlotPart,
        &mut Node,
        Option<&MaterialNode<CooldownMaterial>>,
    )>,
    mut images: Query<(&SlotPart, &mut ImageNode)>,
    mut colors: Query<(&SlotPart, &mut BackgroundColor)>,
    mut texts: Query<(&SlotPart, &mut Text)>,
) {
    let me = player.0;
    let bar = sim.bar_of(me);
    let known = sim.known_spells(me);
    let able = can_act(&*sim, &flow, me);
    let casting = sim.unit(me).and_then(|u| u.cast).map(|c| c.spell);
    let views: Vec<SlotView> = (0..KEY_LABELS.len())
        .map(|i| {
            let Some(&id) = bar.get(i) else {
                return SlotView::default();
            };
            let usable = sim.cooldown(me, id).usable.err();
            SlotView {
                spell: Some(id),
                learned: known.contains(&id),
                locked: matches!(usable, Some(CastError::Silenced | CastError::SchoolLocked)),
                usable,
            }
        })
        .collect();
    let view = |i: usize| views.get(i).copied().unwrap_or_default();
    let held = |i: usize, interaction: &Interaction| {
        (i < crate::combat::SLOT_COUNT && keys.any_pressed(binds.slot_keys(i)))
            || *interaction == Interaction::Pressed
    };

    // Indexed by slot number: query iteration order is not slot order.
    let mut hovered = [false; KEY_LABELS.len()];
    let mut pressed = [false; KEY_LABELS.len()];
    for (Slot(i), it) in &slots {
        if let (Some(h), Some(p)) = (hovered.get_mut(*i), pressed.get_mut(*i)) {
            *h = *it == Interaction::Hovered;
            *p = able && view(*i).learned && held(*i, it);
        }
    }
    let is = |arr: &[bool], i: usize| arr.get(i).copied().unwrap_or(false);
    for (KeyFace(i), mut node, mut border) in &mut faces {
        let v = view(*i);
        let active = v.spell.is_some() && v.spell == casting;
        let lit = active || (v.learned && is(&hovered, *i));
        let want = BorderColor::all(if lit {
            ACCENT
        } else if v.learned {
            KEY_RIM
        } else {
            KEY_SIDE
        });
        if *border != want {
            *border = want;
        }
        let (top, bottom) = key_travel(FACE_INSET, LIP, is(&pressed, *i));
        if node.top != top {
            node.top = top;
            node.bottom = bottom;
        }
    }

    for (SlotPart(i, part), mut img) in &mut images {
        if *part != Part::Icon {
            continue;
        }
        let v = view(*i);
        let cast = v.spell.map(|id| sim.talent_spell(me, id));
        match cast.and_then(|id| sim.content().spell(id)) {
            Some(def) if v.learned => {
                let handle = icons.get(&assets, &def.icon);
                if img.image != handle {
                    img.image = handle;
                }
                img.color = if able { Color::WHITE } else { ICON_STUNNED };
            }
            _ => img.color = Color::NONE,
        }
    }
    for (SlotPart(i, part), mut bg) in &mut colors {
        let v = view(*i);
        if !v.learned {
            bg.0 = Color::NONE;
            continue;
        }
        bg.0 = match part {
            Part::Tint if v.locked => TINT_LOCKED,
            Part::Tint => match v.usable {
                Some(CastError::NotEnoughMana) => TINT_MANA,
                Some(CastError::OutOfRange) => TINT_RANGE,
                _ => Color::NONE,
            },
            Part::Press if is(&pressed, *i) => PRESS,
            Part::Press if is(&hovered, *i) => HOVER,
            _ => Color::NONE,
        };
    }
    for (SlotPart(i, part), mut node, material) in &mut nodes {
        let v = view(*i);
        match part {
            Part::Lock => {
                let want = if v.locked || !v.learned {
                    Display::Flex
                } else {
                    Display::None
                };
                if node.display != want {
                    node.display = want;
                }
            }
            Part::Sweep => {
                let remaining = match v.spell {
                    Some(id) if v.learned => sweep_fraction(&*sim, me, id),
                    _ => 0.0,
                };
                put!(
                    node.display,
                    if remaining > 0.0 {
                        Display::Flex
                    } else {
                        Display::None
                    }
                );
                // Read first: get_mut would mark the asset changed and re-upload it every frame.
                if let Some(handle) = material
                    && materials
                        .get(&handle.0)
                        .is_some_and(|m| m.params.x != remaining)
                    && let Some(mut m) = materials.get_mut(&handle.0)
                {
                    m.params.x = remaining;
                }
            }
            _ => {}
        }
    }
    for (SlotPart(i, part), mut text) in &mut texts {
        let v = view(*i);
        match part {
            Part::Seconds => {
                let ms = match v.spell {
                    Some(id) if v.learned => sim.cooldown(me, id).cd_remaining_ms,
                    _ => 0,
                };
                set_text(
                    &mut text,
                    &if ms > 0 { fmt_secs(ms) } else { String::new() },
                );
            }
            Part::LearnAt => {
                let lv = v
                    .spell
                    .filter(|_| !v.learned)
                    .and_then(|id| sim.content().spell(id))
                    .map_or_else(String::new, |d| format!("Lv {}", d.learn_level));
                set_text(&mut text, &lv);
            }
            _ => {}
        }
    }
}

/// The longer of cooldown and GCD drives the sweep; GCD-only shows no number.
fn sweep_fraction(sim: &dyn SimRead, me: combat_core::EntityId, spell: SpellId) -> f32 {
    let v = sim.cooldown(me, spell);
    let frac = |rem: u32, total: u32| {
        if total == 0 {
            0.0
        } else {
            (rem as f32 / total as f32).clamp(0.0, 1.0)
        }
    };
    if v.cd_remaining_ms > v.gcd_remaining_ms {
        frac(v.cd_remaining_ms, v.cd_total_ms)
    } else {
        frac(v.gcd_remaining_ms, v.gcd_total_ms)
    }
}

/// One tooltip as an IDE hover popup: `fn name(args) -> Ret`, a stats comment and a doc comment.
struct TipContent {
    name: String,
    sig: String,
    stats: String,
    doc: String,
    /// Talent notes, one `// [Talent] text` line each; empty without talents.
    talents: String,
}

/// Return "type" of one effect, as the tooltip and the learned-spell toast print it.
fn effect_ret(effect: &Effect, content: &Content) -> &'static str {
    match effect {
        Effect::Damage {
            execute: Some(_), ..
        } => "Execute",
        Effect::Damage { .. } => "Damage",
        Effect::Absorb { .. } => "Shield",
        Effect::Interrupt { .. } => "Interrupt",
        Effect::Heal { .. } => "Heal",
        Effect::Aura { aura, .. } => match content.aura(aura) {
            Some(a) if a.periodic.as_ref().is_some_and(|p| p.heal) => "Hot",
            Some(a) if a.periodic.is_some() => "Dot",
            Some(a) if a.stun => "Stun",
            Some(a) if a.silence => "Silence",
            Some(a) if a.harmful => "Debuff",
            _ => "Buff",
        },
        Effect::RandomAura { .. } => "Aura",
        Effect::Dispel { .. } => "Dispel",
        Effect::Lockout { .. } => "Lockout",
        Effect::Knockback { .. } => "Knockback",
        Effect::Teleport { .. } => "Teleport",
        Effect::Summon { .. } => "Summon",
        Effect::Resurrect { .. } => "Reboot",
        _ => "Effect",
    }
}

/// `("git_blame", "(target) -> Dot")`; several effects return a tuple.
pub fn spell_signature(d: &SpellDef, content: &Content) -> (String, String) {
    let rets: Vec<&str> = std::iter::once(&d.effect)
        .chain(&d.extra_effects)
        .map(|e| effect_ret(e, content))
        .collect();
    let ret = if rets.len() == 1 {
        rets[0].to_string()
    } else {
        format!("({})", rets.join(", "))
    };
    let arg = match d.target {
        SpellTarget::Ally => "ally = self",
        SpellTarget::DeadAlly => "dead_teammate",
        _ if d.range_m > 0.0 => "target",
        _ => "self",
    };
    (asset_key(&d.name), format!("({arg}) -> {ret}"))
}

/// Stats as the caster pays them: talent perks change mana (or HP), cast time and cooldown.
fn spell_tip(d: &SpellDef, content: &Content, perks: &PerkView) -> TipContent {
    let mut stats = Vec::new();
    let mana = perks.mana_cost(d.id, d.mana);
    let hp = perks.hp_cost(d.mana);
    if hp > 0 {
        stats.push(format!("{hp} HP"));
    } else if mana > 0 {
        stats.push(format!("{mana} mana"));
    }
    stats.push(if d.range_m > 0.0 {
        format!("{:.0} m", d.range_m)
    } else {
        "self".to_string()
    });
    let cast_ms = (i64::from(d.cast_ms) * (100 + perks.cast_time_pct(d.id)).max(0) / 100) as u32;
    stats.push(if d.channel.is_some() {
        // Channel length is authored; haste and cast-time perks do not stretch it.
        format!("{:.1}s channel", d.cast_ms as f32 / 1000.0)
    } else if cast_ms == 0 {
        "instant".to_string()
    } else {
        format!("{:.1}s cast", cast_ms as f32 / 1000.0)
    });
    let cooldown_ms = perks.cooldown(d.id, d.cooldown_ms);
    if cooldown_ms > 0 {
        stats.push(format!("{}s cd", cooldown_ms / 1000));
    }
    if !d.gcd {
        stats.push("off-GCD".to_string());
    }
    let (name, sig) = spell_signature(d, content);
    TipContent {
        name,
        sig,
        stats: stats.join(" | "),
        doc: format!("/// {}", d.description),
        talents: String::new(),
    }
}

/// Coffee Break is a channel with no action-bar slot, so its tooltip is built from its `ChannelDef`.
fn coffee_tip(d: &SpellDef) -> TipContent {
    let (every, pct) = d.channel.map_or((0, 0), |c| (c.interval_ms, c.mana_pct));
    TipContent {
        name: asset_key(&d.name),
        sig: "() -> Mana".to_string(),
        stats: format!(
            "{pct}% mana / {:.0}s | {:.0}s channel | {}s cd",
            every as f32 / 1000.0,
            d.cast_ms as f32 / 1000.0,
            d.cooldown_ms / 1000
        ),
        doc: format!("/// {}", d.description),
        talents: String::new(),
    }
}

fn update_tooltip(
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    forced: Option<Res<DebugHoverSlot>>,
    slots: Query<(&Slot, &Interaction)>,
    attack: Single<&Interaction, With<AttackButton>>,
    coffee: Single<&Interaction, With<CoffeeButton>>,
    rez: Single<&Interaction, With<RezButton>>,
    mut tip: Single<&mut Node, With<Tooltip>>,
    mut lines: Query<(&TipLine, &mut Text)>,
    mut names: Query<&mut TextSpan, (With<TipName>, Without<TipSig>)>,
    mut sigs: Query<&mut TextSpan, (With<TipSig>, Without<TipName>)>,
) {
    let content_db = sim.content();
    let hovered_slot = slots
        .iter()
        .find(|(_, it)| **it != Interaction::None)
        .map(|(Slot(i), _)| *i)
        .or(forced.map(|f| f.0))
        .and_then(|i| sim.bar_of(player.0).get(i).copied())
        .and_then(|id| {
            content_db
                .spell(sim.talent_spell(player.0, id))
                .map(|d| (id, d))
        });
    let hovered_attack = **attack != Interaction::None;
    let hovered_coffee = **coffee != Interaction::None;
    let hovered_rez = **rez != Interaction::None;
    let content = if let Some((id, d)) = hovered_slot {
        if sim.known_spells(player.0).contains(&id) {
            let perks = sim.talent_perks(player.0);
            let mut tip = spell_tip(d, content_db, &PerkView(&perks));
            let (class, picks) = sim.talents_of(player.0);
            tip.talents = content_db
                .talents
                .notes(&class, &picks, id)
                .iter()
                .map(|t| format!("// [{}] {}", t.name, t.text))
                .collect::<Vec<_>>()
                .join("\n");
            Some(tip)
        } else {
            // Unlearned: show only the stub, like an unresolved symbol.
            Some(TipContent {
                name: "locked".into(),
                sig: "() -> !".into(),
                stats: format!("unlocks at level {}", d.learn_level),
                doc: "/// TODO: get promoted first.".into(),
                talents: String::new(),
            })
        }
    } else if hovered_attack {
        sim.unit(player.0)
            .and_then(|u| content_db.unit_def(&u.key))
            .and_then(|d| d.auto_attack.as_ref())
            .map(|a| TipContent {
                name: asset_key(&a.name),
                sig: "(target) -> Damage".to_string(),
                stats: format!(
                    "toggle | {:.0} m | {:.1}s swing",
                    a.range_m,
                    a.swing_ms as f32 / 1000.0
                ),
                doc: format!(
                    "/// {}-{} damage. {}",
                    a.damage_min,
                    a.damage_max,
                    copy::AUTO_ATTACK_FLAVOUR
                ),
                talents: String::new(),
            })
    } else if hovered_coffee {
        content_db.channel_spell().map(coffee_tip)
    } else if hovered_rez {
        sim.resurrect_of(player.0)
            .map(|d| spell_tip(d, content_db, &PerkView(&[])))
    } else {
        None
    };
    let Some(content) = content else {
        put!(tip.display, Display::None);
        return;
    };
    put!(tip.display, Display::Flex);
    // The attack control sits right of the bar, so its tooltip is anchored there instead of mid-bar.
    tip.margin.left = px(if hovered_slot.is_some() {
        -TOOLTIP_W / 2.0
    } else if hovered_rez {
        bar_width() / 2.0 + ATTACK_GAP + 3.0 * ATTACK_PLATE + 2.0 * COFFEE_GAP - TOOLTIP_W
    } else if hovered_coffee {
        bar_width() / 2.0 + ATTACK_GAP + 2.0 * ATTACK_PLATE + COFFEE_GAP - TOOLTIP_W
    } else {
        bar_width() / 2.0 + ATTACK_GAP + ATTACK_PLATE - TOOLTIP_W
    });
    for mut span in &mut names {
        if span.0 != content.name {
            span.0.clone_from(&content.name);
        }
    }
    for mut span in &mut sigs {
        if span.0 != content.sig {
            span.0.clone_from(&content.sig);
        }
    }
    for (TipLine(i), mut text) in &mut lines {
        set_text(
            &mut text,
            match i {
                0 => &content.stats,
                1 => &content.doc,
                _ => &content.talents,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_name_the_effect_kind() {
        let content = Content::from_ron(include_str!("../../../../content/game.ron"))
            .expect("embedded content parses");
        let sig = |id: u32| {
            content
                .spell(SpellId(id))
                .map(|d| spell_signature(d, &content))
        };
        assert_eq!(sig(6), Some(("git_blame".into(), "(target) -> Dot".into())));
        assert_eq!(
            sig(7).map(|s| s.1),
            Some("(ally = self) -> (Heal, Dispel)".into())
        );
        assert_eq!(
            sig(13).map(|s| s.1),
            Some("(dead_teammate) -> Reboot".into())
        );
        assert_eq!(sig(9).map(|s| s.1), Some("(target) -> Stun".into()));
        assert_eq!(sig(10).map(|s| s.1), Some("(target) -> Execute".into()));
    }
}
