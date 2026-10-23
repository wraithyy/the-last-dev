//! Ilmari Wing 1 on the client (docs/ILMARI_09.md B.4): the changelog on Patch Tuesday's wall
//! screens, the red lanyard that marks the android candidate on Story, and the Story | Full
//! switch at the wing door. All presentation: the rules live in `combat_core::raid`.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, PendingCommands, SimReady, SimView};
use crate::floors::{PatchScreen, patch_image};
use crate::game::{GameFlow, zone_label};
use crate::hud::{DIM, KEY_RIM, drop_shadow, label, panel_bg, set_text};
use crate::player::{Player, PlayerBody};
use crate::sprint::SprintHud;
use crate::touch::TouchMode;
use crate::units::Lanyard;
use bevy::prelude::*;
use combat_core::{Command, Event};
use std::collections::HashMap;

/// Red of the Ilmari screens.
const STORY: Color = Color::srgb(0.45, 0.85, 1.0);
const FULL: Color = Color::srgb(1.0, 0.4, 0.3);
/// The switch shows within this many metres of a wing door.
const NEAR_M: f32 = 9.0;
const BOSS: &str = "il_patch_tuesday";

pub struct IlmariWingPlugin;

impl Plugin for IlmariWingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui).add_systems(
            Update,
            (patch_screens, lanyards, difficulty_switch).in_set(SimReady),
        );
    }
}

/// The wall screens show the changelog of whichever patch the boss just installed, and the
/// "failed to install" screen once it falls. A new zone load resets them (the material is new).
fn patch_screens(
    mut reader: MessageReader<CombatEvent>,
    sim: SimView,
    screen: Res<PatchScreen>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<String, Handle<Image>>>,
) {
    let mut show = |key: Option<&str>| {
        let Some(mut mat) = screen.0.as_ref().and_then(|h| materials.get_mut(h)) else {
            return;
        };
        let name = key.unwrap_or("idle").to_string();
        if !cache.contains_key(&name)
            && let Some(img) = patch_image(key)
        {
            cache.insert(name.clone(), images.add(img));
        }
        if let Some(img) = cache.get(&name) {
            mat.base_color_texture = Some(img.clone());
        }
    };
    for CombatEvent(e) in reader.read() {
        match e {
            Event::AuraApplied { target, key } if key.starts_with("patch_") => {
                if sim.unit(*target).is_some_and(|u| u.key == BOSS) {
                    show(Some(key));
                }
            }
            Event::Death { unit, .. } if sim.unit(*unit).is_some_and(|u| u.key == BOSS) => {
                show(Some(""));
            }
            // A reset or a new pull starts from the idle screen again.
            Event::AiState { unit, state }
                if *state == combat_core::AiState::Evade
                    && sim.unit(*unit).is_some_and(|u| u.key == BOSS) =>
            {
                show(None);
            }
            _ => {}
        }
    }
}

/// Story marks the android candidate with a red lanyard; Full leaves it white.
fn lanyards(
    sim: SimView,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lanyards: Query<&MeshMaterial3d<StandardMaterial>, With<Lanyard>>,
) {
    let story = sim.raid_view().story;
    for m in &lanyards {
        if let Some(mut mat) = materials.get_mut(&m.0) {
            let want = if story {
                Color::srgb(0.95, 0.1, 0.1)
            } else {
                Color::srgb(0.92, 0.92, 0.95)
            };
            if mat.base_color != want {
                mat.base_color = want;
            }
        }
    }
}

#[derive(Component)]
struct SwitchRoot;
#[derive(Component)]
struct SwitchTitle;
#[derive(Component)]
struct SwitchCaption;
/// The two segments: true = Story.
#[derive(Component)]
struct SwitchButton(bool);
#[derive(Component)]
struct SwitchLabel(bool);

fn segment(story: bool, text: &str) -> impl Bundle {
    (
        Button,
        SwitchButton(story),
        Node {
            padding: UiRect::axes(px(14), px(5)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(Color::NONE),
        children![(SwitchLabel(story), label(text, 14.0))],
    )
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            SwitchRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(150),
                left: percent(50),
                width: px(440),
                margin: UiRect::left(px(-220)),
                padding: UiRect::axes(px(10), px(6)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(FULL.with_alpha(0.6)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(14),
        ))
        .with_children(|c| {
            c.spawn((SwitchTitle, label("", 12.0), TextColor(DIM)));
            c.spawn(Node {
                column_gap: px(8),
                ..default()
            })
            .with_children(|row| {
                row.spawn(segment(true, "Story"));
                row.spawn(segment(false, "Full"));
            });
            c.spawn((
                SwitchCaption,
                label("", 11.0),
                TextColor(DIM),
                TextLayout::justify(Justify::Center),
            ));
        });
}

fn dist_to(b: &combat_core::Aabb, p: Vec3) -> f32 {
    let dx = (b.min.x - p.x).max(p.x - b.max.x).max(0.0);
    let dz = (b.min.z - p.z).max(p.z - b.max.z).max(0.0);
    (dx * dx + dz * dz).sqrt()
}

/// "Story | Full" above the action bar when standing at a wing's first door (the group lead
/// picks; the others see the call). Solo and pairs default to Story, groups of three to Full.
fn difficulty_switch(
    sim: SimView,
    flow: Res<GameFlow>,
    sprint: Res<SprintHud>,
    touch: Res<TouchMode>,
    body: Single<&PlayerBody, With<Player>>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Interaction, &SwitchButton), Changed<Interaction>>,
    mut root: Single<&mut Node, (With<SwitchRoot>, Without<SwitchButton>)>,
    mut segs: Query<(&SwitchButton, &mut BackgroundColor, &mut BorderColor), Without<SwitchRoot>>,
    mut labels: Query<
        (&SwitchLabel, &mut TextColor),
        (Without<SwitchTitle>, Without<SwitchCaption>),
    >,
    mut title: Single<
        &mut Text,
        (
            With<SwitchTitle>,
            Without<SwitchLabel>,
            Without<SwitchCaption>,
        ),
    >,
    mut caption: Single<
        &mut Text,
        (
            With<SwitchCaption>,
            Without<SwitchLabel>,
            Without<SwitchTitle>,
        ),
    >,
) {
    let content = sim.content();
    let me = sim.player().and_then(|p| sim.unit(p));
    let capped = me.is_some_and(|u| u.alive && u.level >= content.max_level());
    let near = flow
        .exits
        .iter()
        .filter(|e| {
            content
                .raids
                .iter()
                .any(|r| r.ilmari && r.zones.first() == Some(&e.to))
        })
        .map(|e| (e, dist_to(&e.trigger, body.pos)))
        .filter(|(_, d)| *d <= NEAR_M)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e.to.clone());
    let show = capped && !sprint.active && !flow.input_locked() && near.is_some();
    let want = if show { Display::Flex } else { Display::None };
    if root.display != want {
        root.display = want;
    }
    let Some(to) = near.filter(|_| show) else {
        return;
    };
    let story = sim.raid_view().story_next;
    let lead = sim.players().first().copied() == sim.player();
    if lead {
        for (i, b) in &buttons {
            if *i == Interaction::Pressed && b.0 != story {
                pending.0.push(Command::SetIlmariStory { story: b.0 });
            }
        }
    }
    set_text(&mut title, &format!("Next: {}", zone_label(content, &to)));
    let text = match (story, lead) {
        (true, _) => {
            "Story: softer bosses, simpler rules, any group size. Half the loot, half the tokens."
        }
        (false, true) => "Full: the tuned fight, built for three to five. Solo is a bad idea.",
        (false, false) => "Full. The group lead picks the difficulty.",
    };
    set_text(&mut caption, text);
    let bottom = if touch.active { px(94) } else { px(150) };
    if root.bottom != bottom {
        root.bottom = bottom;
    }
    for (b, mut bg, mut border) in &mut segs {
        let on = b.0 == story;
        let tint = if b.0 { STORY } else { FULL };
        bg.0 = if on {
            tint.with_alpha(0.85)
        } else {
            Color::NONE
        };
        *border = BorderColor::all(if on { tint } else { KEY_RIM });
    }
    for (l, mut color) in &mut labels {
        color.0 = if l.0 == story {
            Color::srgb(0.05, 0.05, 0.08)
        } else if lead {
            Color::WHITE
        } else {
            DIM
        };
    }
}
