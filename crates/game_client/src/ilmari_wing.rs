//! Ilmari Wings 1 and 2 on the client (docs/ILMARI_09.md B.4, B.5): the changelog on Patch
//! Tuesday's wall screens, the red lanyard that marks the android candidate on Story, the
//! Story | Full switch at the wing doors, and Wing 2's live boards (the Committee's vote, the
//! Review's grades) and the HR Business Partner's copies, whose nameplates glitch. All
//! presentation: the rules live in `combat_core::raid` and `hr.rs`.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, PendingCommands, SimReady, SimView, UiNotice};
use crate::floors::{LiveBoards, PatchScreen, Sign, live_board_image, patch_image};
use crate::game::{GameFlow, zone_label};
use crate::hud::{Banner, DIM, KEY_RIM, drop_shadow, label, panel_bg, set_text};
use crate::nameplates::PlateName;
use crate::player::{Player, PlayerBody};
use crate::sprint::SprintHud;
use crate::touch::TouchMode;
use crate::units::Lanyard;
use bevy::prelude::*;
use combat_core::{Command, EntityId, Event};
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
        app.init_resource::<Mirrors>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    patch_screens,
                    lanyards,
                    difficulty_switch,
                    vote_board,
                    grade_board,
                    mirror_names,
                )
                    .in_set(SimReady),
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
            mat.emissive_texture = Some(img.clone());
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
                top: px(crate::hud::CHOOSER_TOP),
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
    let (top, bottom) = if touch.active {
        (Val::Auto, px(94))
    } else {
        (px(crate::hud::CHOOSER_TOP), Val::Auto)
    };
    if root.bottom != bottom {
        root.top = top;
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

/// What each ballot slot of the Culture Hub does, as the wall board says it.
fn policy_line(key: &str) -> &'static str {
    match key {
        "pol_rto" => "RETURN TO OFFICE: SLOWER",
        "pol_open_plan" => "OPEN PLAN: +25% DAMAGE TAKEN",
        "pol_efficiency" => "EFFICIENCY: CASTS 30% LONGER",
        "pol_unlimited_pto" => "UNLIMITED VACATION: -20% HP",
        "pol_pizza" => "PIZZA FRIDAY: HASTE, WITH STRINGS",
        _ => "A POLICY",
    }
}

fn set_board(
    handle: Option<&Handle<StandardMaterial>>,
    sign: Sign,
    lines: Option<&[String]>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    let Some(mut mat) = handle.and_then(|h| materials.get_mut(h)) else {
        return;
    };
    // `None` puts the sign's own picture back (the board's idle text).
    let image = match lines {
        Some(l) => live_board_image(sign, l),
        None => crate::floors::sign_image(sign),
    };
    let image = images.add(image);
    mat.base_color_texture = Some(image.clone());
    mat.emissive_texture = Some(image);
}

/// The Culture Hub's vote boards: the ballot's three slots with what each does and how many have
/// been cast, then the policy that passed. Redrawn on every change of the raid view. Banner and
/// notice tell the same to a player who is not looking at a wall.
fn vote_board(
    sim: SimView,
    boards: Res<LiveBoards>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut banners: MessageWriter<Banner>,
    mut last: Local<(Vec<String>, Vec<u32>, String)>,
) {
    let view = sim.raid_view();
    let now = (view.vote.clone(), view.ballots.clone(), view.passed.clone());
    if *last == now {
        return;
    }
    let content = sim.content();
    let name = |k: &str| {
        content.aura(k).map_or(k.to_string(), |a| {
            a.name.trim_start_matches("Policy: ").to_string()
        })
    };
    // A new policy passing is announced once, when the ballot or the pass changes.
    if now.2 != last.2 && !now.2.is_empty() {
        banners.write(Banner {
            title: "POLICY PASSED".into(),
            subtitle: format!(
                "{}. Effective immediately. Retroactive, also.",
                name(&now.2)
            ),
            color: Color::srgb(1.0, 0.75, 0.3),
        });
    }
    let lines: Option<Vec<String>> = if !now.0.is_empty() {
        let mut l = vec!["THE COMMITTEE VOTES".to_string()];
        for (k, key) in now.0.iter().enumerate() {
            let slot = ['A', 'B', 'C'].get(k).copied().unwrap_or('?');
            let n = now.1.get(k).copied().unwrap_or(0);
            l.push(format!("{slot}  {} ({n})", policy_line(key)));
        }
        l.push(if now.2.is_empty() {
            "USE THE BALLOT TERMINALS".to_string()
        } else {
            format!("LAST: {}", name(&now.2).to_uppercase())
        });
        Some(l)
    } else if !now.2.is_empty() {
        Some(vec![
            "THE COMMITTEE HAS VOTED".to_string(),
            "UNANIMOUSLY (3 TO 1, ROUNDED)".to_string(),
            policy_line(&now.2).to_string(),
            "THE BALLOT IS SKIPPED ON STORY".to_string(),
        ])
    } else {
        None
    };
    set_board(
        boards.vote.as_ref(),
        Sign::IlVoteBoard,
        lines.as_deref(),
        &mut materials,
        &mut images,
    );
    *last = now;
}

/// The Review Stage's grade boards: each player's grade of the last review, the one managed out
/// and the best of them, or the idle board before the first review and after a reset.
fn grade_board(
    mut reader: MessageReader<CombatEvent>,
    sim: SimView,
    boards: Res<LiveBoards>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut banners: MessageWriter<Banner>,
    mut notices: MessageWriter<UiNotice>,
) {
    let me = sim.player();
    let name = |id: EntityId| {
        sim.unit(id).map_or("?".to_string(), |u| {
            u.name.to_uppercase().chars().take(14).collect()
        })
    };
    for CombatEvent(e) in reader.read() {
        match e {
            Event::Review {
                grades,
                worst,
                best,
            } => {
                let mut l = vec!["REVIEW: LAST 30 SECONDS".to_string()];
                for (id, pct) in grades.iter().rev().take(5) {
                    let tag = if Some(*id) == *worst {
                        "  MANAGED OUT"
                    } else if Some(*id) == *best {
                        "  EXCEEDS"
                    } else {
                        ""
                    };
                    l.push(format!("{}  {pct}%{tag}", name(*id)));
                }
                l.push("EVERYONE IS A 3".to_string());
                set_board(
                    boards.grades.as_ref(),
                    Sign::IlGradeBoard,
                    Some(&l),
                    &mut materials,
                    &mut images,
                );
                if *worst == me && me.is_some() {
                    banners.write(Banner {
                        title: "MANAGED OUT".into(),
                        subtitle: "Return your badge. And your thermos. Or file an appeal at an HR Portal."
                            .into(),
                        color: Color::srgb(1.0, 0.35, 0.3),
                    });
                } else if *best == me && me.is_some() {
                    notices.write(UiNotice(
                        "Exceeds expectations. We will revisit the goalposts.".into(),
                    ));
                }
            }
            Event::AiState { unit, state }
                if *state == combat_core::AiState::Evade
                    && sim.unit(*unit).is_some_and(|u| u.key == "il_perf_review") =>
            {
                set_board(
                    boards.grades.as_ref(),
                    Sign::IlGradeBoard,
                    None,
                    &mut materials,
                    &mut images,
                );
            }
            _ => {}
        }
    }
}

/// Which player each HR copy wears the name of (`Event::Mirrored`).
#[derive(Resource, Default)]
struct Mirrors(HashMap<EntityId, EntityId>);

/// Every 0.4 s the copy's nameplate re-rolls: some letters become noise, the colour flickers
/// between the enemy red and a cold cyan. The real teammate's plate is steady and blue, so the
/// copy is the one that will not hold still.
fn mirror_names(
    mut reader: MessageReader<CombatEvent>,
    sim: SimView,
    time: Res<Time>,
    mut mirrors: ResMut<Mirrors>,
    mut plates: Query<(&PlateName, &mut Text, &mut TextColor)>,
) {
    for CombatEvent(e) in reader.read() {
        if let Event::Mirrored { mirror, player } = e {
            mirrors.0.insert(*mirror, *player);
        }
    }
    if mirrors.0.is_empty() {
        return;
    }
    let tick = (time.elapsed_secs() / 0.4) as u32;
    for (plate, mut text, mut color) in &mut plates {
        let Some(owner) = mirrors.0.get(&plate.0).and_then(|p| sim.unit(*p)) else {
            continue;
        };
        let level = sim.unit(plate.0).map_or(13, |u| u.level);
        text.0 = format!("{level} {}", glitch(&owner.name, tick));
        color.0 = if tick.is_multiple_of(3) {
            Color::srgb(0.4, 0.95, 1.0)
        } else {
            Color::srgb(1.0, 0.3, 0.3)
        };
    }
}

/// `name` with a few letters swapped for noise, which ones depending on `tick` only.
fn glitch(name: &str, tick: u32) -> String {
    const NOISE: [char; 6] = ['#', '@', '%', '&', '0', '$'];
    name.chars()
        .enumerate()
        .map(|(i, c)| {
            let h = (i as u32 + 1)
                .wrapping_mul(2_654_435_761)
                .wrapping_add(tick.wrapping_mul(40_503));
            if c != ' ' && (h >> 7).is_multiple_of(4) {
                NOISE[(h >> 13) as usize % NOISE.len()]
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::glitch;

    #[test]
    fn a_glitched_name_keeps_its_length_and_changes_between_ticks() {
        let a = glitch("Josef", 3);
        assert_eq!(a.chars().count(), 5);
        assert_eq!(a, glitch("Josef", 3), "the same tick looks the same");
        assert!(
            (0..40).any(|t| glitch("Josef", t) != "Josef"),
            "some tick glitches"
        );
        assert!((0..40).any(|t| glitch("Josef", t) != glitch("Josef", t + 1)));
    }
}
