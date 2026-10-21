//! Seniority, the COBOL Greybeard's resource: a terminal-style bar above the action bar. It is
//! the unit's mana pool (`UnitDef::fixed_mana`), so the player frame hides its own mana row for
//! this class. Hidden for every other class.

use super::{ACCENT_WARM, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::classes::PlayerClass;
use crate::combat::{SimReady, SimUnit, SimView};
use crate::player::Player;
use bevy::prelude::*;

/// Same row as the Exploit Chain pips: clear of the action bar and its braces.
const BOTTOM: f32 = 96.0;
const BAR_W: f32 = 190.0;
const BAR_H: f32 = 14.0;
const GAP: f32 = 8.0;
const LABEL_W: f32 = 140.0;
const WIDTH: f32 = LABEL_W + BAR_W + GAP;
/// Rewrite It in COBOL and friends are worth casting from here up; the bar lights up warm.
const READY_PCT: f32 = 60.0;

#[derive(Component)]
struct SeniorityRoot;
#[derive(Component)]
struct SeniorityFill;
#[derive(Component)]
struct SeniorityLabel;

pub struct SeniorityPlugin;

impl Plugin for SeniorityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, update.in_set(SimReady));
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            SeniorityRoot,
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
                SeniorityLabel,
                label("seniority 0/100", 12.0),
                TextColor(DIM),
                Node {
                    width: px(LABEL_W),
                    ..default()
                },
            ));
            row.spawn((
                Node {
                    width: px(BAR_W),
                    height: px(BAR_H),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(3)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(KEY_RIM),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![(
                    SeniorityFill,
                    Node {
                        width: percent(0),
                        height: percent(100),
                        ..default()
                    },
                    vgrad(ACCENT_WARM, Color::srgb(0.75, 0.3, 0.1)),
                )],
            ));
        });
}

fn update(
    sim: SimView,
    class: Res<PlayerClass>,
    me: Single<&SimUnit, With<Player>>,
    mut root: Single<&mut Node, (With<SeniorityRoot>, Without<SeniorityFill>)>,
    mut fill: Single<&mut Node, (With<SeniorityFill>, Without<SeniorityRoot>)>,
    mut text: Query<(&mut Text, &mut TextColor), With<SeniorityLabel>>,
) {
    let want = if class.0 == "cobol" {
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
    let Some(u) = sim.unit(me.0) else { return };
    let pct = if u.max_mana == 0 {
        0.0
    } else {
        (u.mana as f32 * 100.0 / u.max_mana as f32).clamp(0.0, 100.0)
    };
    let width = percent(pct);
    if fill.width != width {
        fill.width = width;
    }
    for (mut t, mut color) in &mut text {
        set_text(&mut t, &format!("seniority {}/{}", u.mana, u.max_mana));
        let tint = if pct >= READY_PCT { ACCENT_WARM } else { DIM };
        if color.0 != tint {
            color.0 = tint;
        }
    }
}
