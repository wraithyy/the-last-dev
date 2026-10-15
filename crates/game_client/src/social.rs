//! Companions and Code Review Duels on the client. Pets: a primitive rig (`rigs.rs`) per
//! summoned pet that trails its owner, the `~/pets` journal (a button in `~/whoami`) and the
//! pets' chat flavour (the Rubber Duck's real tips, the AI Copilot's confident nonsense).
//! Duels: the right-click menu on a group member, the accept prompt, the arena ring, the
//! 3-2-1 and the result line. The sim (the server, online) decides everything; this only
//! shows it and sends commands.
//!
//! QA hooks: `WOW_PETS=<key>|all` grants (offline) and summons a pet at start,
//! `WOW_PET_LINEUP=1` stands every pet in a row beside the player, `WOW_PETS_OPEN=1` opens the
//! journal, `WOW_DUEL=<secs>` asks the first other group member for a duel at that time and
//! `WOW_DUEL_ACCEPT=1` accepts any request a second later.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{
    CombatEvent, CombatSim, PendingCommands, SimRead, SimReady, SimUnit, SimView, UiNotice,
};
use crate::hud::{ACCENT, DIM, KEY_RIM, KEY_TOP, LogLineMsg, drop_shadow, label, panel_bg};
use crate::net::EmoteRequest;
use crate::player::Player;
use crate::rigs::PrimRig;
use crate::targeting::Target;
use crate::units::Unit;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use combat_core::{Command, DuelOutcome, EntityId, Event};

/// Rig names (`rigs.rs`) for the pet keys in `game.ron`; `PrimRig` needs static names.
const PET_RIGS: [&str; 8] = [
    "pet_rubber_duck",
    "pet_ai_copilot",
    "pet_office_plant",
    "pet_roomba",
    "pet_tamagotchi",
    "pet_stress_ball",
    "pet_server_rack",
    "pet_golden_unicorn",
];
/// Where a pet trots, in the owner's frame (owners face -Z): behind and to the right.
const HEEL: Vec3 = Vec3::new(1.0, 0.0, 0.45);
/// Further than this from its heel spot a pet just appears there (zone change, respawn).
const PET_LEASH_M: f32 = 20.0;
const PET_TOP_SPEED: f32 = 14.0;
const DUCK_TIP_SECS: f32 = 12.0;
const COPILOT_TIP_SECS: f32 = 20.0;
const IDLE_LINE_SECS: f32 = 50.0;
const COPILOT_JOKE_SECS: f32 = 75.0;
/// Friendly units the AI Copilot may "helpfully" target, at most this far away.
const COPILOT_JOKE_RANGE_M: f32 = 15.0;
const PET_CHAT: Color = Color::srgb(1.0, 0.86, 0.45);
const DUEL_CHAT: Color = Color::srgb(0.75, 0.6, 1.0);
const RING_COLOR: Color = Color::srgb(1.0, 0.55, 0.15);

pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerMenu>()
            .init_resource::<DuelUi>()
            .init_resource::<PetJournal>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    qa_start,
                    sync_pets,
                    follow_pets,
                    tamagotchi_mood,
                    social_events,
                    pet_chatter,
                    duel_ui,
                    journal,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// Right-click on another group member (sent by `combat::interact_input`).
#[derive(Message)]
pub struct PlayerMenu(pub EntityId);

/// The `[~/pets]` button in the `~/whoami` header.
#[derive(Component)]
pub struct PetsButton;

#[derive(Component)]
struct Pet {
    owner: EntityId,
    key: String,
}

/// The red screen a Tamagotchi shows while its owner is dead.
#[derive(Component)]
struct TamaRed(EntityId);

#[derive(Component)]
struct ArenaRing(EntityId, EntityId);

#[derive(Resource, Default)]
struct DuelUi {
    /// The group member the right-click menu is for, and where it opened.
    menu: Option<(EntityId, Vec2)>,
    /// Who asked us, and seconds left to answer.
    prompt: Option<(EntityId, f32)>,
    /// Countdown seconds left, then the "fight" flash.
    countdown: Option<f32>,
}

#[derive(Resource, Default)]
struct PetJournal {
    open: bool,
    /// Owned pets and the active one, as last drawn.
    drawn: Option<(Vec<String>, String)>,
}

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct MenuButton(bool);
#[derive(Component)]
struct PromptRoot;
#[derive(Component)]
struct PromptText;
#[derive(Component)]
struct PromptButton(bool);
#[derive(Component)]
struct CountdownText;
#[derive(Component)]
struct JournalRoot;
#[derive(Component)]
struct JournalList;
#[derive(Component)]
struct JournalClose;
#[derive(Component)]
struct PetButton(String);

fn rig_of(key: &str) -> &'static str {
    PET_RIGS
        .iter()
        .find(|r| &r[4..] == key)
        .copied()
        .unwrap_or("pet_unknown")
}

fn name_of(sim: &dyn SimRead, id: EntityId) -> String {
    sim.unit(id)
        .map_or_else(|| "Someone".into(), |u| u.name.clone())
}

fn pet_name(sim: &dyn SimRead, key: &str) -> String {
    sim.content()
        .pet(key)
        .map_or_else(|| key.to_string(), |p| p.name.clone())
}

fn button(text: &str) -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(10), px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(KEY_TOP),
        children![label(text, 13.0)],
    )
}

fn spawn_ui(mut commands: Commands) {
    let panel = |top: Val, left: Val| {
        (
            Node {
                position_type: PositionType::Absolute,
                top,
                left,
                padding: UiRect::all(px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(40),
        )
    };
    commands
        .spawn((MenuRoot, panel(px(0), px(0))))
        .with_children(|c| {
            c.spawn((MenuButton(true), button("Request code review")));
            c.spawn((MenuButton(false), button("Cancel")));
        });
    commands
        .spawn((PromptRoot, panel(percent(22), percent(36))))
        .with_children(|c| {
            c.spawn((PromptText, label("", 14.0)));
            c.spawn(Node {
                column_gap: px(10),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((PromptButton(true), button("Approve the review")));
                row.spawn((PromptButton(false), button("Won't fix")));
            });
        });
    commands.spawn((
        CountdownText,
        Node {
            position_type: PositionType::Absolute,
            top: percent(26),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        label("", 56.0),
        TextLayout::justify(Justify::Center),
        TextColor(RING_COLOR),
        GlobalZIndex(35),
    ));
    commands
        .spawn((
            JournalRoot,
            crate::economy::window_node(Some(630.0), None, 400.0),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("~/pets", 14.0), TextColor(DIM)));
                h.spawn((JournalClose, Button, label("[x]", 14.0)));
            });
            c.spawn((
                JournalList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                },
            ));
        });
}

/// QA hooks, once the player exists.
fn qa_start(
    time: Res<Time>,
    mut done: Local<(bool, bool, bool)>,
    mut sim: ResMut<CombatSim>,
    net: Option<Res<crate::net::NetWorld>>,
    mut pending: ResMut<PendingCommands>,
    mut journal: ResMut<PetJournal>,
    mut commands: Commands,
    player: Single<(&SimUnit, &Transform), With<Player>>,
) {
    let (me, tf) = *player;
    if !done.0 {
        done.0 = true;
        if let Ok(v) = std::env::var("WOW_PETS") {
            let keys: Vec<String> = if v == "all" {
                sim.0.content().pets.iter().map(|p| p.key.clone()).collect()
            } else {
                vec![v]
            };
            if net.is_none() {
                for k in &keys {
                    sim.0.grant_pet(me.0, k);
                }
            }
            if let Some(k) = keys.first() {
                pending.0.push(Command::SummonPet { pet: k.clone() });
            }
        }
        journal.open = std::env::var_os("WOW_PETS_OPEN").is_some();
    }
    // After the zone placed the player at its spawn.
    if !done.2 && time.elapsed_secs() > 2.0 {
        done.2 = true;
        if std::env::var_os("WOW_PET_LINEUP").is_some() {
            for (k, rig) in PET_RIGS.iter().enumerate() {
                let side = tf.rotation * Vec3::X;
                let ahead = tf.rotation * Vec3::NEG_Z;
                let at = tf.translation + ahead * 3.0 + side * (k as f32 - 3.5) * 0.9;
                commands.spawn((
                    PrimRig(rig),
                    Transform::from_translation(at)
                        .with_rotation(tf.rotation * Quat::from_rotation_y(std::f32::consts::PI)),
                    Visibility::default(),
                ));
            }
        }
    }
    // Online the other player may join later than we do, so the request waits for the time.
    let at = std::env::var("WOW_DUEL")
        .ok()
        .and_then(|v| v.parse::<f32>().ok());
    if let (Some(at), Some(net), false) = (at, net.as_deref(), done.1)
        && time.elapsed_secs() >= at
        && let Some(other) = net.players().iter().find(|p| **p != me.0)
    {
        done.1 = true;
        pending.0.push(Command::DuelRequest { target: *other });
    }
}

/// One rig per summoned pet of every group member; gone when dismissed or the owner leaves.
fn sync_pets(
    mut commands: Commands,
    sim: SimView,
    pets: Query<(Entity, &Pet)>,
    owners: Query<(&SimUnit, &Transform), Or<(With<Player>, With<Unit>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let wanted: Vec<(EntityId, String)> = sim
        .players()
        .iter()
        .filter_map(|p| sim.unit(*p).map(|u| (*p, u.pet.clone())))
        .filter(|(_, k)| !k.is_empty())
        .collect();
    for (e, pet) in &pets {
        if !wanted.iter().any(|(o, k)| *o == pet.owner && *k == pet.key) {
            commands.entity(e).despawn();
        }
    }
    for (owner, key) in wanted {
        if pets.iter().any(|(_, p)| p.owner == owner && p.key == key) {
            continue;
        }
        let Some((_, otf)) = owners.iter().find(|(s, _)| s.0 == owner) else {
            continue;
        };
        let at = otf.translation + otf.rotation * HEEL;
        let mut e = commands.spawn((
            Pet {
                owner,
                key: key.clone(),
            },
            PrimRig(rig_of(&key)),
            Transform::from_translation(at).with_rotation(otf.rotation),
            Visibility::default(),
        ));
        if key == "tamagotchi" {
            let mesh = meshes.add(Cuboid::new(0.135, 0.105, 0.012));
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.1, 0.08),
                emissive: LinearRgba::rgb(3.0, 0.2, 0.1),
                ..default()
            });
            e.with_children(|c| {
                c.spawn((
                    TamaRed(owner),
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(0.0, 0.24, -0.172),
                    Visibility::Hidden,
                ));
            });
        }
    }
}

/// Simple steering towards the heel spot: faster the further behind, hopping while it runs.
fn follow_pets(
    time: Res<Time>,
    mut pets: Query<(&Pet, &mut Transform), Without<SimUnit>>,
    owners: Query<(&SimUnit, &Transform), (Or<(With<Player>, With<Unit>)>, Without<Pet>)>,
) {
    let (dt, t) = (time.delta_secs(), time.elapsed_secs());
    for (pet, mut tf) in &mut pets {
        let Some((_, otf)) = owners.iter().find(|(s, _)| s.0 == pet.owner) else {
            continue;
        };
        let heel = otf.translation + otf.rotation * HEEL;
        let to = Vec3::new(heel.x - tf.translation.x, 0.0, heel.z - tf.translation.z);
        let d = to.length();
        if d > PET_LEASH_M {
            tf.translation = heel;
            continue;
        }
        let speed = ((d - 0.25) * 3.5).clamp(0.0, PET_TOP_SPEED);
        let step = if d > 1e-3 {
            to / d * speed * dt
        } else {
            Vec3::ZERO
        };
        tf.translation.x += step.x;
        tf.translation.z += step.z;
        // Phase by owner so two pets never hop in lockstep.
        let phase = pet.owner as f32;
        let hop = if speed > 0.6 {
            (t * 11.0 + phase).sin().abs() * 0.12
        } else {
            0.0
        };
        tf.translation.y = otf.translation.y + hop;
        let face = if speed > 0.6 {
            Quat::from_rotation_y(f32::atan2(-to.x, -to.z))
        } else {
            // Idle: look where the owner looks, with a little sway.
            otf.rotation * Quat::from_rotation_y((t * 1.3 + phase).sin() * 0.25)
        };
        tf.rotation = tf.rotation.slerp(face, (dt * 8.0).min(1.0));
    }
}

fn tamagotchi_mood(sim: SimView, mut screens: Query<(&TamaRed, &mut Visibility)>) {
    for (t, mut vis) in &mut screens {
        let dead = sim.unit(t.0).is_some_and(|u| !u.alive);
        *vis = if dead {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Chat lines, notices, the prompt and the arena ring from pet and duel events.
fn social_events(
    mut events: MessageReader<CombatEvent>,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut ui: ResMut<DuelUi>,
    mut lines: MessageWriter<LogLineMsg>,
    mut notices: MessageWriter<UiNotice>,
    mut commands: Commands,
    rings: Query<(Entity, &ArenaRing)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let me = player.0;
    let mut say = |text: String, color: Color| {
        lines.write(LogLineMsg(text, color));
    };
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::PetChanged { unit, pet } if *unit == me => {
                let text = if pet.is_empty() {
                    "Your companion went back to its desk.".to_string()
                } else {
                    format!("{} is now following you.", pet_name(&*sim, pet))
                };
                say(text, PET_CHAT);
            }
            Event::PetLearned { unit, pet } if *unit == me => say(
                format!(
                    "New companion: {}. Open ~/pets from ~/whoami (I).",
                    pet_name(&*sim, pet)
                ),
                PET_CHAT,
            ),
            Event::PetLooted { items, .. } => say(
                format!("Roomba Drone vacuumed up: {}.", items.join(", ")),
                PET_CHAT,
            ),
            Event::Death { unit, .. }
                if *unit == me && sim.unit(me).is_some_and(|u| u.pet == "tamagotchi") =>
            {
                say(
                    "Tamagotchi Build Pipeline: *BEEEEP* BUILD FAILED. Blame: you.".into(),
                    PET_CHAT,
                )
            }
            Event::DuelRequested { from, to } => {
                if *to == me {
                    ui.prompt = Some((*from, combat_core::duel::DUEL_ASK_MS as f32 / 1000.0));
                    say(
                        format!(
                            "{} requested your review on their PR (duel).",
                            name_of(&*sim, *from)
                        ),
                        DUEL_CHAT,
                    );
                } else if *from == me {
                    say(
                        format!(
                            "You asked {} for a code review. Waiting for them to stop pretending to be busy.",
                            name_of(&*sim, *to)
                        ),
                        DUEL_CHAT,
                    );
                }
            }
            Event::DuelCountdown {
                a,
                b,
                center,
                radius_m,
                ms,
            } => {
                if *b == me {
                    ui.prompt = None;
                }
                if *a == me || *b == me {
                    ui.countdown = Some(*ms as f32 / 1000.0);
                }
                let ring = meshes.add(Torus::new(radius_m - 0.12, radius_m + 0.12));
                let mat = materials.add(StandardMaterial {
                    base_color: RING_COLOR,
                    emissive: LinearRgba::rgb(4.0, 1.6, 0.3),
                    unlit: true,
                    ..default()
                });
                commands.spawn((
                    ArenaRing(*a, *b),
                    Mesh3d(ring),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(center.x, center.y + 0.05, center.z)
                        .with_scale(Vec3::new(1.0, 0.2, 1.0)),
                ));
                say(
                    format!(
                        "{} vs {}: code review in 3... Leave the orange circle and the PR is closed.",
                        name_of(&*sim, *a),
                        name_of(&*sim, *b)
                    ),
                    DUEL_CHAT,
                );
            }
            Event::DuelEnded {
                a,
                b,
                winner,
                outcome,
                comments,
                streak,
            } => {
                for (e, r) in &rings {
                    if r.0 == *a && r.1 == *b {
                        commands.entity(e).despawn();
                    }
                }
                if *a == me || *b == me {
                    ui.prompt = None;
                    ui.countdown = None;
                }
                let (na, nb) = (name_of(&*sim, *a), name_of(&*sim, *b));
                let loser = winner.map(|w| if w == *a { *b } else { *a });
                let (nw, nl) = (
                    winner.map(|w| name_of(&*sim, w)).unwrap_or_default(),
                    loser.map(|l| name_of(&*sim, l)).unwrap_or_default(),
                );
                let mut text = match outcome {
                    DuelOutcome::Won => {
                        format!("{nw} approved {nl}'s PR after {comments} comments.")
                    }
                    DuelOutcome::Forfeit => {
                        format!("{nl} closed the PR without merging. {nw} wins by forfeit.")
                    }
                    DuelOutcome::Declined => {
                        format!("{nb} marked {na}'s review request as won't fix.")
                    }
                    DuelOutcome::TimedOut => format!("{nb} left {na}'s review request on read."),
                    DuelOutcome::Cancelled => {
                        format!(
                            "The code review between {na} and {nb} was postponed. Something is on fire."
                        )
                    }
                };
                if *streak > 1 {
                    text.push_str(&format!(" ({streak} approvals in a row.)"));
                }
                say(text, DUEL_CHAT);
            }
            Event::DuelRefused { reason } => {
                notices.write(UiNotice(reason.message().into()));
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct Chatter {
    tip_in: f32,
    idle_in: f32,
    joke_in: f32,
    cursor: usize,
    squeaked: bool,
}

/// The local player's pet talks: tips in combat, idle lines out of it, Copilot jokes.
fn pet_chatter(
    time: Res<Time>,
    sim: SimView,
    player: Single<(&SimUnit, &Transform), With<Player>>,
    mut target: ResMut<Target>,
    units: Query<(Entity, &SimUnit, &Unit, &Transform)>,
    mut emotes: MessageReader<EmoteRequest>,
    mut st: Local<Chatter>,
    mut lines: MessageWriter<LogLineMsg>,
) {
    let (me, my_tf) = *player;
    let Some(u) = sim.unit(me.0) else { return };
    let Some(def) = sim.content().pet(&u.pet) else {
        emotes.clear();
        return;
    };
    let dt = time.delta_secs();
    let pick = |lines: &[String], k: usize| lines.get(k % lines.len().max(1)).cloned();
    let mut say = |text: String| {
        lines.write(LogLineMsg(format!("{}: {text}", def.name), PET_CHAT));
    };
    for EmoteRequest(e) in emotes.read() {
        if def.key == "ai_copilot" {
            st.cursor += 1;
            if st.cursor.is_multiple_of(2) {
                say(format!(
                    "autocompleted your emote: \"{} ...and that's why we are rewriting it in Rust.\"",
                    e.text()
                ));
            }
        }
    }
    if !u.in_combat {
        st.squeaked = false;
    }
    st.tip_in -= dt;
    st.idle_in -= dt;
    st.joke_in -= dt;
    if u.in_combat {
        if def.key == "stress_ball" && !st.squeaked && u.hp * 100 < u.max_hp * 30 {
            st.squeaked = true;
            say("*SQUEAK* (you are about to die)".into());
        }
        if st.tip_in > 0.0 {
            return;
        }
        let foe = target
            .0
            .and_then(|e| units.get(e).ok())
            .and_then(|(_, su, _, _)| sim.unit(su.0));
        match def.key.as_str() {
            "rubber_duck" => {
                if let Some(tip) = combat_core::duck_tip(sim.content(), u, foe) {
                    say(tip);
                }
                st.tip_in = DUCK_TIP_SECS;
            }
            "ai_copilot" => {
                st.cursor += 1;
                if let Some(l) = pick(&def.lines, st.cursor) {
                    say(l);
                }
                st.tip_in = COPILOT_TIP_SECS;
            }
            _ => st.tip_in = DUCK_TIP_SECS,
        }
        return;
    }
    // A fresh fight gets its first tip quickly.
    st.tip_in = st.tip_in.min(3.0);
    if def.key == "ai_copilot" && st.joke_in <= 0.0 {
        st.joke_in = COPILOT_JOKE_SECS;
        let friend = units
            .iter()
            .filter(|(_, su, unit, tf)| {
                unit.alive
                    && !unit.hostile
                    && !sim.players().contains(&su.0)
                    && tf.translation.distance(my_tf.translation) <= COPILOT_JOKE_RANGE_M
            })
            .min_by(|a, b| {
                let d = |t: &Transform| t.translation.distance(my_tf.translation);
                d(a.3).total_cmp(&d(b.3))
            });
        if let Some((e, su, ..)) = friend {
            target.0 = Some(e);
            say(format!(
                "I targeted {} for you. Based on your history, you wanted to attack them. You're welcome.",
                name_of(&*sim, su.0)
            ));
        }
        return;
    }
    if st.idle_in <= 0.0 {
        // Spread first lines out so a fresh session does not open with a chorus.
        if st.idle_in > -1.0 && time.elapsed_secs() > 8.0 {
            st.cursor += 1;
            if def.key != "ai_copilot"
                && let Some(l) = pick(&def.lines, st.cursor)
            {
                say(l);
            }
        }
        st.idle_in = IDLE_LINE_SECS;
    }
}

fn duel_ui(
    time: Res<Time>,
    sim: SimView,
    mut ui: ResMut<DuelUi>,
    mut menus: MessageReader<PlayerMenu>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut pending: ResMut<PendingCommands>,
    menu_buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    prompt_buttons: Query<(&Interaction, &PromptButton), Changed<Interaction>>,
    mut menu: Single<&mut Node, (With<MenuRoot>, Without<PromptRoot>, Without<CountdownText>)>,
    mut prompt: Single<&mut Node, (With<PromptRoot>, Without<MenuRoot>, Without<CountdownText>)>,
    mut prompt_text: Single<&mut Text, (With<PromptText>, Without<CountdownText>)>,
    mut count: Single<
        (&mut Node, &mut Text),
        (With<CountdownText>, Without<MenuRoot>, Without<PromptRoot>),
    >,
    mut auto_accept: Local<f32>,
    ui_scale: Res<UiScale>,
) {
    let dt = time.delta_secs();
    for PlayerMenu(id) in menus.read() {
        let at = window.cursor_position().unwrap_or(Vec2::new(400.0, 300.0));
        ui.menu = Some((*id, at));
    }
    for (i, b) in &menu_buttons {
        if *i == Interaction::Pressed {
            if let (true, Some((id, _))) = (b.0, ui.menu) {
                pending.0.push(Command::DuelRequest { target: id });
            }
            ui.menu = None;
        }
    }
    let mut answer = None;
    for (i, b) in &prompt_buttons {
        if *i == Interaction::Pressed {
            answer = Some(b.0);
        }
    }
    if ui.prompt.is_some() && std::env::var_os("WOW_DUEL_ACCEPT").is_some() {
        *auto_accept += dt;
        if *auto_accept > 1.0 {
            answer = Some(true);
        }
    } else {
        *auto_accept = 0.0;
    }
    if let (Some(accept), Some(_)) = (answer, ui.prompt) {
        pending.0.push(Command::DuelRespond { accept });
        ui.prompt = None;
    }
    if let Some((_, left)) = ui.prompt.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            ui.prompt = None;
        }
    }
    // A menu for someone who left closes itself.
    if ui.menu.is_some_and(|(id, _)| !sim.players().contains(&id)) {
        ui.menu = None;
    }
    match ui.menu {
        Some((_, at)) => {
            // The cursor is in logical pixels; UI values are scaled by `UiScale`.
            let s = ui_scale.0.max(0.01);
            menu.display = Display::Flex;
            menu.left = px(at.x / s);
            menu.top = px(at.y / s);
        }
        None => menu.display = Display::None,
    }
    match ui.prompt {
        Some((from, left)) => {
            prompt.display = Display::Flex;
            prompt_text.0 = format!(
                "{} wants you to review their PR.\nA code review duel: first to 1 HP loses. {:.0}s to answer.",
                name_of(&*sim, from),
                left.max(0.0)
            );
        }
        None => prompt.display = Display::None,
    }
    let (node, text) = &mut *count;
    match ui.countdown.as_mut() {
        Some(left) => {
            *left -= dt;
            node.display = Display::Flex;
            text.0 = if *left > 0.0 {
                format!("{}", left.ceil() as u32)
            } else {
                "MERGE CONFLICT! FIGHT!".into()
            };
            if *left < -1.2 {
                ui.countdown = None;
            }
        }
        None => node.display = Display::None,
    }
}

/// `~/pets`: every pet in the content, owned ones with a summon or dismiss button.
fn journal(
    mut commands: Commands,
    sim: SimView,
    player: Single<&SimUnit, With<Player>>,
    mut state: ResMut<PetJournal>,
    open_buttons: Query<&Interaction, (With<PetsButton>, Changed<Interaction>)>,
    close: Query<&Interaction, (With<JournalClose>, Changed<Interaction>)>,
    pet_buttons: Query<(&Interaction, &PetButton), Changed<Interaction>>,
    mut root: Single<&mut Node, With<JournalRoot>>,
    list: Single<Entity, With<JournalList>>,
    mut pending: ResMut<PendingCommands>,
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<crate::input::Paused>,
) {
    let key = !paused.0 && binds.just(&keys, crate::keybinds::Action::Pets);
    if key || open_buttons.iter().any(|i| *i == Interaction::Pressed) {
        state.open = !state.open;
    }
    if close.iter().any(|i| *i == Interaction::Pressed) {
        state.open = false;
    }
    for (i, b) in &pet_buttons {
        if *i == Interaction::Pressed {
            pending.0.push(Command::SummonPet { pet: b.0.clone() });
        }
    }
    root.display = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    let Some(me) = sim.unit(player.0) else { return };
    let now = (me.pets.clone(), me.pet.clone());
    if !state.open || state.drawn.as_ref() == Some(&now) {
        return;
    }
    state.drawn = Some(now);
    let list = *list;
    commands.entity(list).despawn_related::<Children>();
    let owned = me.pets.len();
    let total = sim.content().pets.len();
    commands.entity(list).with_children(|c| {
        c.spawn((
            label(
                format!("{owned}/{total} collected. One follows you at a time."),
                12.0,
            ),
            TextColor(DIM),
        ));
        for p in &sim.content().pets {
            let has = me.pets.contains(&p.key);
            let active = me.pet == p.key;
            c.spawn(Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                row_gap: px(2),
                ..default()
            })
            .insert(BorderColor::all(if active {
                ACCENT
            } else {
                KEY_RIM.with_alpha(0.4)
            }))
            .with_children(|row| {
                row.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|head| {
                    let color = if has {
                        Color::WHITE
                    } else {
                        DIM.with_alpha(0.6)
                    };
                    head.spawn((label(&p.name, 14.0), TextColor(color)));
                    if active {
                        head.spawn((PetButton(String::new()), button("dismiss")));
                    } else if has {
                        head.spawn((PetButton(p.key.clone()), button("summon")));
                    } else {
                        head.spawn((label("locked", 12.0), TextColor(DIM.with_alpha(0.6))));
                    }
                });
                row.spawn((label(&p.blurb, 11.0), TextColor(DIM)));
                // Vendor pets show the carrier's live price, so a rebalance needs no text edit.
                let price = sim
                    .content()
                    .economy
                    .consumable(&p.carrier)
                    .filter(|_| p.drops.is_empty())
                    .map_or_else(String::new, |c| format!(", {} Kudos", c.price));
                row.spawn((
                    label(format!("Source: {}{price}", p.source), 11.0),
                    TextColor(ACCENT.with_alpha(0.8)),
                ));
            });
        }
    });
}
