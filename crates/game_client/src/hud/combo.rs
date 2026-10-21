//! Exploit Chain, the Security Engineer's combo meter: five keycap pips above the action bar,
//! lit for the stacks of the `exploit_chain` aura on the current target, plus a stealth tag
//! while Tailgating hides you. Hidden for every other class.

use super::{ACCENT, ACCENT_WARM, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::classes::PlayerClass;
use crate::combat::{SimReady, SimUnit, SimView};
use crate::player::Player;
use crate::targeting::Target;
use bevy::prelude::*;

/// The aura the finishers consume; content owns the stack cap.
const AURA: &str = "exploit_chain";
const PIPS: usize = 5;
/// Clear of the action bar (20 px margin + 70 px bar) and its flanking braces.
const BOTTOM: f32 = 96.0;
const PIP_W: f32 = 22.0;
const PIP_H: f32 = 14.0;
const GAP: f32 = 4.0;
/// Label column width, so the pips stay centred under the bar's middle.
const LABEL_W: f32 = 130.0;
const WIDTH: f32 = LABEL_W + PIPS as f32 * (PIP_W + GAP) + 70.0;

#[derive(Component)]
struct ComboRoot;
#[derive(Component)]
struct ComboPip(usize);
#[derive(Component)]
struct ComboLabel;
#[derive(Component)]
struct StealthTag;

pub struct ComboPlugin;

impl Plugin for ComboPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, update.in_set(SimReady));
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            ComboRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(BOTTOM),
                left: percent(50),
                margin: UiRect::left(px(-WIDTH / 2.0)),
                width: px(WIDTH),
                align_items: AlignItems::Center,
                column_gap: px(GAP),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(5),
        ))
        .with_children(|row| {
            row.spawn((
                ComboLabel,
                label("exploit_chain 0/5", 12.0),
                TextColor(DIM),
                Node {
                    width: px(LABEL_W),
                    ..default()
                },
            ));
            for k in 0..PIPS {
                row.spawn((
                    ComboPip(k),
                    Node {
                        width: px(PIP_W),
                        height: px(PIP_H),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    vgrad(KEY_TOP_HI, KEY_TOP),
                ));
            }
            row.spawn((StealthTag, label("", 12.0), TextColor(ACCENT)));
        });
}

fn update(
    sim: SimView,
    class: Res<PlayerClass>,
    target: Res<Target>,
    units: Query<&SimUnit>,
    me: Single<&SimUnit, With<Player>>,
    mut root: Single<&mut Node, With<ComboRoot>>,
    mut pips: Query<(&ComboPip, &mut BorderColor, &mut BackgroundGradient)>,
    mut text: Query<&mut Text, (With<ComboLabel>, Without<StealthTag>)>,
    mut tag: Query<&mut Text, (With<StealthTag>, Without<ComboLabel>)>,
    mut shown: Local<Option<usize>>,
) {
    let want = if class.0 == "security" {
        Display::Flex
    } else {
        Display::None
    };
    if root.display != want {
        root.display = want;
    }
    if want == Display::None {
        return;
    }
    let stacks = target
        .0
        .and_then(|e| units.get(e).ok())
        .and_then(|su| sim.unit(su.0))
        .and_then(|u| u.auras.iter().find(|a| a.key == AURA))
        .map_or(0, |a| usize::from(a.stacks));
    // Gradients have no cheap equality: restyle only when the count changes.
    if *shown != Some(stacks) {
        *shown = Some(stacks);
        for (pip, mut border, mut fill) in &mut pips {
            let lit = pip.0 < stacks;
            border.set_all(if lit { ACCENT_WARM } else { KEY_RIM });
            *fill = if lit {
                vgrad(ACCENT_WARM, Color::srgb(0.75, 0.3, 0.1))
            } else {
                vgrad(KEY_TOP_HI, KEY_TOP)
            };
        }
    }
    for mut t in &mut text {
        set_text(&mut t, &format!("exploit_chain {stacks}/{PIPS}"));
    }
    let stealthed = sim.unit(me.0).is_some_and(|u| {
        u.auras.iter().any(|a| {
            sim.content()
                .aura(&a.key)
                .is_some_and(|d| d.stealth_aggro_pct > 0)
        })
    });
    for mut t in &mut tag {
        set_text(&mut t, if stealthed { "[stealth]" } else { "" });
    }
}
