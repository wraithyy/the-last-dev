//! The always-visible Kudos wallet: a gold "K" coin and the balance beside the bottom-right dock,
//! kept apart from the purple XP bar so the currency never reads as progress. Every gain floats
//! up from it as "+45 Kudos" in gold; hovering it shows the flavour line.

use super::goods::SP_GOLD;
use crate::combat::{SimReady, SimView};
use crate::copy::{KUDOS_FLAVOUR, kudos, thousands};
use crate::hud::{DIM, PANEL_EDGE, label, panel_bg, set_text};
use bevy::prelude::*;

const FLOAT_SECS: f32 = 1.8;
const FLOAT_RISE: f32 = 34.0;
/// The wallet sits on the dock's row, left of the chat/log/dps buttons.
const WALLET_BOTTOM: f32 = 15.0;
const WALLET_RIGHT: f32 = 164.0;

#[derive(Component)]
struct Wallet;
#[derive(Component)]
struct WalletText;
#[derive(Component)]
struct WalletFlavour;
#[derive(Component)]
struct KudosFloat(f32);

pub struct WalletPlugin;

impl Plugin for WalletPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_wallet)
            .add_systems(Update, (update_wallet, drift).chain().in_set(SimReady));
    }
}

/// A round gold coin with a dark "K" (no glyph asset needed, so it reads at any UI scale).
pub fn coin(size: f32) -> impl Bundle {
    (
        Node {
            width: px(size),
            height: px(size),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(percent(50)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(SP_GOLD),
        BorderColor::all(Color::srgb(0.55, 0.38, 0.05)),
        children![(
            Text::new("K"),
            TextFont::from_font_size(size * 0.7),
            TextColor(Color::srgb(0.25, 0.15, 0.0)),
        )],
    )
}

fn spawn_wallet(mut commands: Commands) {
    commands.spawn((
        Wallet,
        Button,
        Node {
            position_type: PositionType::Absolute,
            right: px(WALLET_RIGHT),
            bottom: px(WALLET_BOTTOM),
            padding: UiRect::axes(px(6), px(2)),
            column_gap: px(5),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(9)),
            ..default()
        },
        BorderColor::all(SP_GOLD.mix(&PANEL_EDGE, 0.5)),
        panel_bg(),
        GlobalZIndex(20),
        children![
            coin(14.0),
            (WalletText, label("Kudos 0", 12.0), TextColor(SP_GOLD)),
            (
                WalletFlavour,
                label(KUDOS_FLAVOUR, 10.0),
                TextColor(DIM),
                Node {
                    display: Display::None,
                    ..default()
                },
            ),
        ],
    ));
}

fn update_wallet(
    mut commands: Commands,
    sim: SimView,
    wallet: Single<(Entity, &Interaction), With<Wallet>>,
    mut text: Single<&mut Text, With<WalletText>>,
    mut flavour: Single<&mut Node, With<WalletFlavour>>,
    mut last: Local<Option<u32>>,
) {
    let (entity, hover) = *wallet;
    flavour.display = if *hover == Interaction::None {
        Display::None
    } else {
        Display::Flex
    };
    let Some(sp) = sim.player().and_then(|p| sim.unit(p)).map(|u| u.sp) else {
        *last = None;
        return;
    };
    // Floats only for gains seen live, not for the balance a save or restart loads.
    if let Some(prev) = *last
        && sp > prev
    {
        commands.spawn((
            KudosFloat(0.0),
            label(format!("+{}", kudos(sp - prev)), 16.0),
            TextColor(SP_GOLD),
            Node {
                position_type: PositionType::Absolute,
                right: px(WALLET_RIGHT + 4.0),
                bottom: px(WALLET_BOTTOM + 22.0),
                ..default()
            },
            GlobalZIndex(70),
        ));
        commands.entity(entity).insert(BorderColor::all(SP_GOLD));
    } else if *last != Some(sp) {
        commands
            .entity(entity)
            .insert(BorderColor::all(SP_GOLD.mix(&PANEL_EDGE, 0.5)));
    }
    if *last != Some(sp) {
        set_text(&mut text, &format!("Kudos {}", thousands(sp)));
        *last = Some(sp);
    }
}

fn drift(
    mut commands: Commands,
    time: Res<Time>,
    mut floats: Query<(Entity, &mut KudosFloat, &mut Node, &mut TextColor)>,
) {
    for (e, mut f, mut node, mut color) in &mut floats {
        f.0 += time.delta_secs();
        let t = (f.0 / FLOAT_SECS).min(1.0);
        node.bottom = px(WALLET_BOTTOM + 22.0 + FLOAT_RISE * t);
        color.0 = SP_GOLD.with_alpha(1.0 - t * t);
        if t >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}
