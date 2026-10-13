//! Action bar: slots are bound to `Content::action_bar()`; everything on a slot
//! (icon, tint, sweep, seconds) is derived from the sim's `CooldownView`.

use super::{
    ACCENT, ACCENT_WARM, DIM, DebugHoverSlot, IconCache, KEY_RIM, KEY_SIDE, KEY_TOP, KEY_TOP_HI,
    PANEL_EDGE, asset_key, drop_shadow, fmt_secs, gloss, label, panel_bg, set_text, vgrad,
};
use crate::combat::{
    CombatSim, PendingCommands, SimReady, SimUnit, UiNotice, cast_slot, toggle_auto_attack,
};
use crate::config::*;
use crate::copy;
use crate::input::Paused;
use crate::player::Player;
use crate::targeting::Target;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use combat_core::{CastError, Effect, SpellId};

const KEY_LABELS: [&str; 4] = ["1", "2", "3", "4"];
const TINT_MANA: Color = Color::srgba(0.1, 0.2, 0.95, 0.5);
const TINT_RANGE: Color = Color::srgba(0.85, 0.1, 0.1, 0.5);
const ATTACK_ICON: &str = "nitpick";
const SWING_BAR: Color = Color::srgb(0.95, 0.85, 0.5);
const ATTACK_EDGE: Color = Color::srgb(0.75, 0.4, 0.26);
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
/// Clearance between the attack control and the left brace.
const ATTACK_GAP: f32 = 30.0;
const TOOLTIP_W: f32 = 310.0;
const TOOLTIP_BOTTOM: f32 = 108.0;

/// Radial clock-wipe overlay; see `assets/shaders/cooldown_sweep.wgsl`.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct CooldownMaterial {
    /// x = fraction of the cooldown still remaining. A Vec4 because WebGL2
    /// needs 16-byte aligned uniforms.
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
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Icon,
    Tint,
    Sweep,
    Seconds,
    Press,
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

pub struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_bar).add_systems(
            Update,
            (
                click_slots,
                click_attack,
                update_slots,
                update_attack,
                update_tooltip,
            )
                .chain()
                .in_set(SimReady),
        );
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
fn legend(key: &str) -> impl Bundle {
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
        children![(label(key, 11.0), TextColor(Color::srgb(0.9, 0.95, 1.0)))],
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

fn spawn_bar(mut commands: Commands, mut materials: ResMut<Assets<CooldownMaterial>>) {
    let width = bar_width();
    let height = bar_height();
    commands
        .spawn((
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
                            label("", 22.0),
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                top: px(10),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            TextLayout::justify(Justify::Center),
                            TextColor(Color::srgb(1.0, 0.95, 0.7)),
                        ));
                        face.spawn((SlotPart(i, Part::Press), fill, BackgroundColor(Color::NONE)));
                        face.spawn(legend(key));
                    });
                });
            }
        });

    spawn_attack(&mut commands);

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
        });
}

/// The auto attack is a wider, warm accent keycap kept apart from the spell row, like an Esc key.
fn spawn_attack(commands: &mut Commands) {
    let left = -(bar_width() / 2.0 + ATTACK_GAP + ATTACK_PLATE);
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: px(BAR_BOTTOM),
            left: percent(50),
            margin: UiRect::left(px(left)),
            width: px(ATTACK_PLATE),
            height: px(bar_height()),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(4),
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                AttackRing,
                Node {
                    width: px(ATTACK_PLATE),
                    height: px(ATTACK_H),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
                BorderColor::all(PANEL_EDGE),
                vgrad(Color::srgb(0.2, 0.08, 0.05), Color::srgb(0.06, 0.03, 0.03)),
                drop_shadow(),
            ))
            .with_children(|ring| {
                ring.spawn((
                    AttackButton,
                    Button,
                    Node {
                        overflow: Overflow::clip(),
                        ..face_node(1.0, ATTACK_LIP, 0.0)
                    },
                    BorderColor::all(ATTACK_EDGE),
                    vgrad(Color::srgb(0.92, 0.45, 0.24), Color::srgb(0.58, 0.2, 0.12)),
                ))
                .with_children(|b| {
                    b.spawn((
                        AttackIcon,
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
                        AttackGlow,
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ));
                    b.spawn(legend("T"));
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
                    AttackSwing,
                    Node {
                        height: percent(100),
                        width: percent(0),
                        ..default()
                    },
                    vgrad(SWING_BAR, Color::srgb(0.6, 0.45, 0.15)),
                )],
            ));
        });
}

fn click_slots(
    paused: Res<Paused>,
    sim: Res<CombatSim>,
    mut pending: ResMut<PendingCommands>,
    player: Single<&SimUnit, With<Player>>,
    slots: Query<(&Slot, &Interaction), Changed<Interaction>>,
) {
    if paused.0 {
        return;
    }
    for (Slot(i), interaction) in &slots {
        if *interaction == Interaction::Pressed {
            cast_slot(&sim.0, &mut pending, player.0, *i);
        }
    }
}

fn click_attack(
    paused: Res<Paused>,
    sim: Res<CombatSim>,
    target: Res<Target>,
    mut pending: ResMut<PendingCommands>,
    mut notices: MessageWriter<UiNotice>,
    player: Single<&SimUnit, With<Player>>,
    button: Single<&Interaction, (With<AttackButton>, Changed<Interaction>)>,
) {
    if !paused.0 && **button == Interaction::Pressed {
        toggle_auto_attack(
            &sim.0,
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
    sim: Res<CombatSim>,
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
    let handle = icons.get(&assets, ATTACK_ICON);
    if icon.image != handle {
        icon.image = handle;
    }
    let state = sim.0.unit(player.0);
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
        .and_then(|u| sim.0.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or(0, |a| a.swing_ms);
    swing.width = match state {
        Some(u) if on && swing_ms > 0 && u.swing_remaining_ms > 0 => {
            let done = 1.0 - (u.swing_remaining_ms as f32 / swing_ms as f32).clamp(0.0, 1.0);
            percent(100.0 * done)
        }
        _ => percent(0),
    };
}

/// Top and bottom offsets of a keycap face; pressing sinks it by `TRAVEL`.
fn key_travel(inset: f32, lip: f32, pressed: bool) -> (Val, Val) {
    let t = if pressed { TRAVEL } else { 0.0 };
    (px(inset + t), px(lip - t))
}

fn update_slots(
    sim: Res<CombatSim>,
    keys: Res<ButtonInput<KeyCode>>,
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
    let bar = sim.0.content().action_bar();
    let casting = sim.0.unit(me).and_then(|u| u.cast).map(|c| c.spell);
    let spell_of = |i: usize| bar.get(i).copied();
    let held = |i: usize, interaction: &Interaction| {
        crate::combat::CAST_KEYS
            .get(i)
            .is_some_and(|k| keys.any_pressed(*k))
            || *interaction == Interaction::Pressed
    };

    // Indexed by slot number: query iteration order is not slot order.
    let mut hovered = [false; KEY_LABELS.len()];
    let mut pressed = [false; KEY_LABELS.len()];
    for (Slot(i), it) in &slots {
        if let (Some(h), Some(p)) = (hovered.get_mut(*i), pressed.get_mut(*i)) {
            *h = *it == Interaction::Hovered;
            *p = held(*i, it);
        }
    }
    for (KeyFace(i), mut node, mut border) in &mut faces {
        let active = spell_of(*i).is_some() && spell_of(*i) == casting;
        let lit = active || hovered.get(*i).copied().unwrap_or(false);
        let want = BorderColor::all(if lit { ACCENT } else { KEY_RIM });
        if *border != want {
            *border = want;
        }
        let (top, bottom) = key_travel(FACE_INSET, LIP, pressed.get(*i).copied().unwrap_or(false));
        if node.top != top {
            node.top = top;
            node.bottom = bottom;
        }
    }

    for (SlotPart(i, part), mut img) in &mut images {
        if *part != Part::Icon {
            continue;
        }
        match spell_of(*i).and_then(|id| sim.0.content().spell(id)) {
            Some(def) => {
                let handle = icons.get(&assets, &def.icon);
                if img.image != handle {
                    img.image = handle;
                }
                img.color = Color::WHITE;
            }
            None => img.color = Color::NONE,
        }
    }
    for (SlotPart(i, part), mut bg) in &mut colors {
        let Some(id) = spell_of(*i) else {
            bg.0 = Color::NONE;
            continue;
        };
        let view = sim.0.cooldown(me, id);
        bg.0 = match part {
            Part::Tint => match view.usable {
                Err(CastError::NotEnoughMana) => TINT_MANA,
                Err(CastError::OutOfRange) => TINT_RANGE,
                _ => Color::NONE,
            },
            Part::Press if pressed.get(*i).copied().unwrap_or(false) => PRESS,
            Part::Press if hovered.get(*i).copied().unwrap_or(false) => HOVER,
            _ => Color::NONE,
        };
    }
    for (SlotPart(i, part), mut node, material) in &mut nodes {
        if *part != Part::Sweep {
            continue;
        }
        let remaining = spell_of(*i).map_or(0.0, |id| sweep_fraction(&sim, me, id));
        node.display = if remaining > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
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
    for (SlotPart(i, part), mut text) in &mut texts {
        if *part != Part::Seconds {
            continue;
        }
        let ms = spell_of(*i).map_or(0, |id| sim.0.cooldown(me, id).cd_remaining_ms);
        set_text(
            &mut text,
            &if ms > 0 { fmt_secs(ms) } else { String::new() },
        );
    }
}

/// The longer of cooldown and GCD drives the sweep; GCD-only shows no number.
fn sweep_fraction(sim: &CombatSim, me: combat_core::EntityId, spell: SpellId) -> f32 {
    let v = sim.0.cooldown(me, spell);
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
}

fn spell_tip(d: &combat_core::SpellDef) -> TipContent {
    let ret = match d.effect {
        Effect::Damage { .. } => "Damage",
        Effect::Absorb { .. } => "Shield",
        Effect::Interrupt { .. } => "Interrupt",
    };
    let arg = if d.range_m > 0.0 { "target" } else { "self" };
    let mut stats = Vec::new();
    if d.mana > 0 {
        stats.push(format!("{} mana", d.mana));
    }
    stats.push(if d.range_m > 0.0 {
        format!("{:.0} m", d.range_m)
    } else {
        "self".to_string()
    });
    stats.push(if d.cast_ms == 0 {
        "instant".to_string()
    } else {
        format!("{:.1}s cast", d.cast_ms as f32 / 1000.0)
    });
    if d.cooldown_ms > 0 {
        stats.push(format!("{}s cd", d.cooldown_ms / 1000));
    }
    TipContent {
        name: asset_key(&d.name),
        sig: format!("({arg}) -> {ret}"),
        stats: stats.join(" | "),
        doc: format!("/// {}", d.description),
    }
}

fn update_tooltip(
    sim: Res<CombatSim>,
    player: Single<&SimUnit, With<Player>>,
    forced: Option<Res<DebugHoverSlot>>,
    slots: Query<(&Slot, &Interaction)>,
    attack: Single<&Interaction, With<AttackButton>>,
    mut tip: Single<&mut Node, With<Tooltip>>,
    mut lines: Query<(&TipLine, &mut Text)>,
    mut names: Query<&mut TextSpan, (With<TipName>, Without<TipSig>)>,
    mut sigs: Query<&mut TextSpan, (With<TipSig>, Without<TipName>)>,
) {
    let hovered_spell = slots
        .iter()
        .find(|(_, it)| **it != Interaction::None)
        .map(|(Slot(i), _)| *i)
        .or(forced.map(|f| f.0))
        .and_then(|i| sim.0.content().action_bar().get(i).copied())
        .and_then(|id| sim.0.content().spell(id));
    let hovered_attack = **attack != Interaction::None;
    let content = if let Some(d) = hovered_spell {
        Some(spell_tip(d))
    } else if hovered_attack {
        sim.0
            .unit(player.0)
            .and_then(|u| sim.0.content().unit_def(&u.key))
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
            })
    } else {
        None
    };
    let Some(content) = content else {
        tip.display = Display::None;
        return;
    };
    tip.display = Display::Flex;
    // The attack control sits left of the bar, so its tooltip is anchored there instead of mid-bar.
    tip.margin.left = px(if hovered_spell.is_none() {
        -(bar_width() / 2.0 + ATTACK_GAP + ATTACK_PLATE)
    } else {
        -TOOLTIP_W / 2.0
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
            if *i == 0 {
                &content.stats
            } else {
                &content.doc
            },
        );
    }
}
