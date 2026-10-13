//! Action bar: slots are bound to `Content::action_bar()`; everything on a slot
//! (icon, tint, sweep, seconds) is derived from the sim's `CooldownView`.

use super::{BAR_BG, BRONZE, GOLD, IconCache, fmt_secs, label, set_text};
use crate::combat::{
    CombatSim, PendingCommands, SimReady, SimUnit, UiNotice, cast_slot, toggle_auto_attack,
};
use crate::config::*;
use crate::input::Paused;
use crate::player::Player;
use crate::targeting::Target;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use combat_core::{CastError, SpellId};

const KEY_LABELS: [&str; 4] = ["1", "2", "3", "4"];
const KEY_CODES: [KeyCode; 4] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
];
const TINT_MANA: Color = Color::srgba(0.1, 0.2, 0.95, 0.5);
const TINT_RANGE: Color = Color::srgba(0.85, 0.1, 0.1, 0.5);
const ATTACK_ICON: &str = "nitpick";
const ATTACK_PULSE: Color = Color::srgb(1.0, 0.35, 0.2);
const SWING_BAR: Color = Color::srgb(0.95, 0.85, 0.5);
const PRESS: Color = Color::srgba(1.0, 1.0, 1.0, 0.35);

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
#[derive(Component)]
struct Tooltip;
/// The Attack (auto attack) button, left of slot 1.
#[derive(Component)]
struct AttackButton;
#[derive(Component)]
struct AttackIcon;
#[derive(Component)]
struct AttackSwing;

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

fn spawn_bar(mut commands: Commands, mut materials: ResMut<Assets<CooldownMaterial>>) {
    let n = KEY_LABELS.len() + 1;
    let width = n as f32 * ACTION_SLOT + (n as f32 - 1.0) * ACTION_GAP + 16.0;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(18),
                left: percent(50),
                margin: UiRect::left(px(-width / 2.0)),
                padding: UiRect::all(px(6)),
                column_gap: px(ACTION_GAP),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(super::PANEL),
        ))
        .with_children(|bar| {
            bar.spawn((
                AttackButton,
                Button,
                Node {
                    width: px(ACTION_SLOT),
                    height: px(ACTION_SLOT),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(4)),
                    margin: UiRect::right(px(ACTION_GAP)),
                    ..default()
                },
                BorderColor::all(BRONZE),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|b| {
                b.spawn((
                    AttackIcon,
                    ImageNode::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                ));
                b.spawn((
                    AttackSwing,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        bottom: px(0),
                        height: px(4),
                        width: percent(0),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(SWING_BAR),
                ));
                b.spawn((
                    label("T", 12.0),
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(1),
                        right: px(3),
                        ..default()
                    },
                ));
            });
            for (i, key) in KEY_LABELS.iter().enumerate() {
                bar.spawn((
                    Slot(i),
                    Button,
                    Node {
                        width: px(ACTION_SLOT),
                        height: px(ACTION_SLOT),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(BRONZE),
                    BackgroundColor(BAR_BG),
                ))
                .with_children(|slot| {
                    let fill = Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    };
                    slot.spawn((SlotPart(i, Part::Icon), ImageNode::default(), fill.clone()));
                    slot.spawn((
                        SlotPart(i, Part::Tint),
                        fill.clone(),
                        BackgroundColor(Color::NONE),
                    ));
                    slot.spawn((
                        SlotPart(i, Part::Sweep),
                        MaterialNode(materials.add(CooldownMaterial { params: Vec4::ZERO })),
                        Node {
                            display: Display::None,
                            ..fill.clone()
                        },
                    ));
                    slot.spawn((
                        SlotPart(i, Part::Seconds),
                        label("", 20.0),
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            top: px(14),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        TextLayout::justify(Justify::Center),
                    ));
                    slot.spawn((SlotPart(i, Part::Press), fill, BackgroundColor(Color::NONE)));
                    slot.spawn((
                        label(*key, 12.0),
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(1),
                            right: px(3),
                            ..default()
                        },
                    ));
                });
            }
        });
    commands.spawn((
        Tooltip,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(100),
            left: percent(50),
            margin: UiRect::left(px(-120)),
            width: px(240),
            padding: UiRect::all(px(8)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(GOLD),
        BackgroundColor(Color::srgba(0.03, 0.03, 0.06, 0.95)),
        GlobalZIndex(20),
        label("", 13.0),
    ));
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

/// Border flashes while auto attack is on, like WoW's attack icon; the thin bar shows the swing timer.
fn update_attack(
    time: Res<Time>,
    sim: Res<CombatSim>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    player: Single<&SimUnit, With<Player>>,
    mut button: Single<&mut BorderColor, With<AttackButton>>,
    mut icon: Single<&mut ImageNode, With<AttackIcon>>,
    mut swing: Single<(&mut Node, &mut BackgroundColor), With<AttackSwing>>,
) {
    let handle = icons.get(&assets, ATTACK_ICON);
    if icon.image != handle {
        icon.image = handle;
    }
    let state = sim.0.unit(player.0);
    let on = state.is_some_and(|u| u.auto_attack);
    **button = BorderColor::all(if on {
        let k = 0.5 + 0.5 * (time.elapsed_secs() * 7.0).sin();
        BRONZE.mix(&ATTACK_PULSE, k)
    } else {
        BRONZE
    });
    let swing_ms = state
        .and_then(|u| sim.0.content().unit_def(&u.key))
        .and_then(|d| d.auto_attack.as_ref())
        .map_or(0, |a| a.swing_ms);
    let (node, _) = &mut *swing;
    match state {
        Some(u) if on && swing_ms > 0 && u.swing_remaining_ms > 0 => {
            node.display = Display::Flex;
            let done = 1.0 - (u.swing_remaining_ms as f32 / swing_ms as f32).clamp(0.0, 1.0);
            node.width = percent(100.0 * done);
        }
        _ => node.display = Display::None,
    }
}

fn update_slots(
    sim: Res<CombatSim>,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut materials: ResMut<Assets<CooldownMaterial>>,
    player: Single<&SimUnit, With<Player>>,
    mut slots: Query<(&Slot, &Interaction, &mut BorderColor)>,
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
        KEY_CODES.get(i).is_some_and(|k| keys.pressed(*k)) || *interaction == Interaction::Pressed
    };

    for (Slot(i), interaction, mut border) in &mut slots {
        let active = spell_of(*i).is_some() && spell_of(*i) == casting;
        *border = BorderColor::all(if active { GOLD } else { BRONZE });
        let _ = interaction;
    }
    let pressed: Vec<bool> = slots.iter().map(|(Slot(i), it, _)| held(*i, it)).collect();

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

fn update_tooltip(
    sim: Res<CombatSim>,
    slots: Query<(&Slot, &Interaction)>,
    mut tip: Single<(&mut Node, &mut Text), With<Tooltip>>,
) {
    let hovered = slots
        .iter()
        .find(|(_, it)| **it != Interaction::None)
        .and_then(|(Slot(i), _)| sim.0.content().action_bar().get(*i).copied())
        .and_then(|id| sim.0.content().spell(id));
    let (node, text) = &mut *tip;
    let Some(d) = hovered else {
        node.display = Display::None;
        return;
    };
    node.display = Display::Flex;
    let cast = if d.cast_ms == 0 {
        "Instant".to_string()
    } else {
        format!("{:.1} sec cast", d.cast_ms as f32 / 1000.0)
    };
    let cd = if d.cooldown_ms > 0 {
        format!("\n{} sec cooldown", d.cooldown_ms / 1000)
    } else {
        String::new()
    };
    let range = if d.range_m > 0.0 {
        format!("{:.0} m range", d.range_m)
    } else {
        "Self".to_string()
    };
    let mana = if d.mana > 0 {
        format!("{} mana  ", d.mana)
    } else {
        String::new()
    };
    set_text(
        text,
        &format!("{}\n{mana}{range}\n{cast}{cd}\n{}", d.name, d.description),
    );
}
