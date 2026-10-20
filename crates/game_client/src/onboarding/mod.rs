//! "First Day": the onboarding for a class with no save yet (offline only, replayable from the
//! Esc menu, skippable). It walks the player through the campus onboarding corner (HR bot,
//! training dummy, burnt-out colleague, all authored in content) and hands off to the intern.
//! Every step completes from real input or sim events (`steps.rs`); the panel shows the
//! current step and pulses an outline on the HUD element it talks about.
//!
//! QA hooks: `WOW_ONBOARDING=1` starts it even without the save flag (screenshots disable
//! saves), `WOW_ONBOARDING_STEP=<n>` jumps to step n (0-based).
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod steps;

use crate::camera::OrbitCamera;
use crate::combat::{CombatEvent, CombatSim, SimReady, SimUnit, Talked};
use crate::economy::EconomyUi;
use crate::game::{EnterZone, GameFlow};
use crate::hud::{
    ACCENT, ACCENT_WARM, Banner, DIM, KEY_RIM, KEY_TOP, KEY_TOP_HI, PANEL_EDGE, label, panel_bg,
    set_text, vgrad,
};
use crate::input::{CareerOpen, Paused, QuestWindows};
use crate::keybinds::{Action, Keybinds};
use crate::net::{GameMode, Lobby};
use crate::player::{Player, PlayerBody};
use crate::targeting::Target;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{Effect, EntityId, Event, QuestStatus, Role, SpellId, SpellTarget};
use steps::{Ctx, STEPS, Signal, Step, Tutorial, Window};

pub const QUEST: &str = "mandatory_compliance";
/// Every new hire starts in Car Park P3, where the First Day corner is.
pub const STARTER: &str = crate::floors::carpark::ZONE;
const DUMMY: &str = "training_dummy";
const COLLEAGUE: &str = "burnt_out_colleague";
const INTERN: &str = "intern";
const VIDEO_SPELL: SpellId = SpellId(1390);
const ZONE_SPELL: SpellId = SpellId(1391);
/// Before the loot step the dummy never drops below this share: the module just restarts.
const DUMMY_FLOOR_PCT: u32 = 30;
/// The colleague is "burnt out" to this share while a healer is asked to heal them.
const COLLEAGUE_HP_PCT: u32 = 40;
const FLASH_SECS: f32 = 3.5;
/// Yaw (rad) or zoom (m) change that counts as having looked around.
const LOOK_YAW: f32 = 0.6;
const LOOK_ZOOM: f32 = 1.0;
const PANEL_W: f32 = 470.0;

/// HUD elements the onboarding can point at; the HUD modules tag their nodes with it.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spot {
    Slot(u8),
    AutoAttack,
    Tracker,
    TargetFrame,
    Minimap,
    Mount,
    /// A touch-layout button injecting this action.
    Touch(Action),
}

struct Run {
    tut: Tutorial,
    /// Where the player and the camera were when the current step began.
    from: Vec3,
    cam: (f32, f32),
    started_step: Option<usize>,
    /// Compliance Zones of the dummy still to land (telegraph ids).
    zones: Vec<u32>,
    flash: Option<(String, f32)>,
}

/// The loaded class's onboarding state; `pending` is what the save stores.
#[derive(Resource, Default)]
pub struct Onboarding {
    pub pending: bool,
    run: Option<Run>,
    /// Ask to (re)start once the conditions allow it.
    want: bool,
    /// The new-hire trip to P3 was asked for (once per class load).
    redirected: bool,
}

impl Onboarding {
    /// The save of a freshly loaded class decided whether onboarding is still due.
    pub fn class_loaded(&mut self, pending: bool) {
        self.pending = pending;
        self.run = None;
        self.want = pending;
        self.redirected = false;
    }
}

#[derive(Component)]
struct PanelRoot;
#[derive(Component)]
struct StepText;
#[derive(Component)]
struct TitleText;
#[derive(Component)]
struct HowText;
#[derive(Component)]
struct QuipText;
#[derive(Component)]
struct ProgressCell(usize);
#[derive(Component)]
struct SkipButton;
#[derive(Component)]
struct ReplayMenu;
#[derive(Component)]
struct ReplayButton;

pub struct OnboardingPlugin;

impl Plugin for OnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Onboarding>()
            .add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (start_and_stop, observe, keep_props_useful, panel, highlight)
                    .chain()
                    .in_set(SimReady),
            )
            .add_systems(Update, replay_menu)
            .add_systems(Update, hold_night.in_set(SimReady));
        if std::env::var_os("WOW_ONBOARDING").is_some() {
            app.add_systems(PostStartup, |mut o: ResMut<Onboarding>| o.want = true);
        }
    }
}

fn find_unit(sim: &CombatSim, key: &str) -> Option<EntityId> {
    // Prefer the living one: a corpse lingers next to its respawn.
    let mut best = None;
    for u in sim.0.units().filter(|u| u.key == key) {
        if u.alive {
            return Some(u.id);
        }
        best = Some(u.id);
    }
    best
}

/// Bar slot, name and learn level of the first class spell whose effect matches.
fn bar_spell(
    sim: &CombatSim,
    player: EntityId,
    want: impl Fn(&combat_core::SpellDef) -> bool,
) -> Option<(u8, String, u8)> {
    let c = sim.0.content();
    let unit = sim.0.unit(player)?;
    let bar = c.bar_for_unit(&unit.key);
    bar.iter().enumerate().find_map(|(i, id)| {
        let s = c.spell(*id).filter(|s| want(s))?;
        Some((u8::try_from(i).ok()?, s.name.clone(), s.learn_level.max(1)))
    })
}

fn knows_slot(sim: &CombatSim, player: EntityId, slot: u8) -> bool {
    let c = sim.0.content();
    let Some(unit) = sim.0.unit(player) else {
        return false;
    };
    let Some(id) = c.bar_for_unit(&unit.key).get(usize::from(slot)) else {
        return false;
    };
    sim.0.known_spells(player).contains(id)
}

fn is_interrupt(s: &combat_core::SpellDef) -> bool {
    matches!(s.effect, Effect::Interrupt { .. })
}

fn new_run(
    sim: &CombatSim,
    player: EntityId,
    touch: bool,
    body: &PlayerBody,
    cam: &OrbitCamera,
) -> Option<Run> {
    let role = sim.0.class_of(player)?.role;
    let can_interrupt = bar_spell(sim, player, is_interrupt)
        .is_some_and(|(slot, ..)| knows_slot(sim, player, slot));
    let mut tut = Tutorial::new(role, can_interrupt);
    if touch {
        tut.skip_window(Window::Sheet);
    }
    if let Some(n) = std::env::var("WOW_ONBOARDING_STEP")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        tut.step = n.min(STEPS.len() - 1);
    }
    Some(Run {
        tut,
        from: body.pos,
        cam: (cam.yaw(), cam.zoom()),
        started_step: None,
        zones: Vec::new(),
        flash: None,
    })
}

fn start_and_stop(
    mut onb: ResMut<Onboarding>,
    mode: Res<GameMode>,
    flow: Res<GameFlow>,
    sim: Res<CombatSim>,
    touch: Res<TouchMode>,
    body: Single<&PlayerBody, With<Player>>,
    cam: Single<&OrbitCamera>,
    skip: Query<&Interaction, (Changed<Interaction>, With<SkipButton>)>,
    mut banners: MessageWriter<Banner>,
    mut enter: MessageWriter<EnterZone>,
) {
    // Co-op skips it: the server owns that world and the group would wait on one player.
    if mode.is_online() {
        onb.run = None;
        return;
    }
    if onb.run.is_some() && skip.iter().any(|i| *i == Interaction::Pressed) {
        onb.run = None;
        onb.pending = false;
        onb.want = false;
        banners.write(Banner {
            title: "Onboarding skipped".into(),
            subtitle: "Marked complete. Nobody checks. Replay it from the Esc menu.".into(),
            color: DIM,
        });
        return;
    }
    // A new hire who woke up on the campus (the default start) is sent to P3 first, once.
    if onb.want && onb.run.is_none() && !flow.zone.is_empty() && flow.zone != STARTER {
        if sends_new_hire_to_p3(&onb, &flow.zone, flow.input_locked()) {
            onb.redirected = true;
            enter.write(EnterZone {
                zone: STARTER.into(),
                from: None,
            });
        }
        return;
    }
    if onb.want && onb.run.is_none() && flow.zone == STARTER && !flow.input_locked() {
        let Some(p) = sim.0.player() else { return };
        onb.run = new_run(&sim, p, touch.active, &body, &cam);
        onb.want = false;
    }
    let finished = onb.run.as_ref().is_some_and(|r| r.tut.done());
    if finished {
        onb.run = None;
        onb.pending = false;
        banners.write(Banner {
            title: "Onboarding complete".into(),
            subtitle: "You are now a fully onboarded resource. Your laptop request is pending."
                .into(),
            color: Color::srgb(0.4, 1.0, 0.5),
        });
    }
}

/// A new hire (onboarding due, not running yet) anywhere but P3 is sent there, once per class
/// load; the caller has already left co-op out (the server owns that world).
fn sends_new_hire_to_p3(onb: &Onboarding, zone: &str, locked: bool) -> bool {
    onb.want
        && onb.run.is_none()
        && !zone.is_empty()
        && zone != STARTER
        && !locked
        && !onb.redirected
}

/// Turns this frame's input, state and sim events into step signals.
fn observe(
    mut onb: ResMut<Onboarding>,
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    target: Res<Target>,
    windows: Res<QuestWindows>,
    eco: Res<EconomyUi>,
    career: Res<CareerOpen>,
    body: Single<&PlayerBody, With<Player>>,
    cam: Single<&OrbitCamera>,
    units: Query<&SimUnit>,
    mut events: MessageReader<CombatEvent>,
    mut talked: MessageReader<Talked>,
) {
    let Some(run) = onb.run.as_mut() else {
        events.clear();
        talked.clear();
        return;
    };
    // Only the P3 corner exists for it; elsewhere it waits.
    if flow.zone != STARTER {
        events.clear();
        talked.clear();
        return;
    }
    let Some(p) = sim.0.player() else { return };
    if run.started_step != Some(run.tut.step) {
        run.started_step = Some(run.tut.step);
        run.from = body.pos;
        run.cam = (cam.yaw(), cam.zoom());
    }
    let dummy = find_unit(&sim, DUMMY);
    let colleague = find_unit(&sim, COLLEAGUE);
    let mut sig: Vec<Signal> = Vec::new();

    let walked = Vec2::new(body.pos.x - run.from.x, body.pos.z - run.from.z).length();
    sig.push(Signal::Moved(walked));
    let yaw = (cam.yaw() - run.cam.0).abs();
    if yaw > LOOK_YAW || (cam.zoom() - run.cam.1).abs() > LOOK_ZOOM {
        sig.push(Signal::CameraMoved);
    }
    let quest = sim.0.quest_status(QUEST);
    if matches!(
        quest,
        QuestStatus::Active | QuestStatus::Ready | QuestStatus::Done
    ) {
        sig.push(Signal::QuestAccepted);
    }
    if quest == QuestStatus::Done {
        sig.push(Signal::QuestTurnedIn);
    }
    let targeted = target.0.and_then(|e| units.get(e).ok()).map(|u| u.0);
    if targeted.is_some() && targeted == dummy {
        sig.push(Signal::TargetedDummy);
    }
    if sim.0.unit(p).is_some_and(|u| u.auto_attack) {
        sig.push(Signal::AutoAttackOn);
    }
    if windows.map {
        sig.push(Signal::Opened(Window::Map));
    }
    if eco.sheet {
        sig.push(Signal::Opened(Window::Sheet));
    }
    if career.0 {
        sig.push(Signal::Opened(Window::Career));
    }
    let content = sim.0.content();
    if sim
        .0
        .unit(p)
        .is_some_and(|u| u.auras.iter().any(|a| content.is_mount(&a.key)))
    {
        sig.push(Signal::Mounted);
    }
    // The intern's own chain counts too: talking to him and accepting is the hand-off.
    if sim.0.quest_status("onboarding") != QuestStatus::Available
        && sim.0.quest_status("onboarding") != QuestStatus::Locked
    {
        sig.push(Signal::TalkedToIntern);
    }
    for Talked(id) in talked.read() {
        if sim.0.unit(*id).is_some_and(|u| u.key == INTERN) {
            sig.push(Signal::TalkedToIntern);
        }
    }

    let bar = sim
        .0
        .unit(p)
        .map(|u| content.bar_for_unit(&u.key).to_vec())
        .unwrap_or_default();
    let is_dummy = |id: &EntityId| Some(*id) == dummy;
    let (mut zone_landed, mut zone_hit) = (false, false);
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastSucceeded { caster, spell, .. } if *caster == p => {
                let slot = bar.iter().position(|b| {
                    *b == *spell
                        || content
                            .talents
                            .spell_for(&class_key(&sim, p), &talents(&sim, p), *b)
                            == *spell
                });
                if let Some(i) = slot.and_then(|i| u8::try_from(i).ok()) {
                    sig.push(Signal::CastSlot(i));
                }
            }
            Event::CastSucceeded { caster, spell, .. }
            | Event::CastFailed { caster, spell, .. }
                if is_dummy(caster) && *spell == VIDEO_SPELL =>
            {
                sig.push(Signal::VideoEnded { interrupted: false });
            }
            Event::CastInterrupted {
                caster, spell, by, ..
            } if is_dummy(caster) && *spell == VIDEO_SPELL => {
                sig.push(Signal::VideoEnded {
                    interrupted: *by == Some(p),
                });
            }
            Event::Damage {
                target, absorbed, ..
            } if *target == p && *absorbed > 0 => sig.push(Signal::Soaked),
            Event::Damage {
                target,
                spell: Some(s),
                ..
            } if *target == p && *s == ZONE_SPELL => zone_hit = true,
            Event::Damage {
                source,
                target,
                spell: Some(_),
                ..
            } if *source == p && is_dummy(target) => {
                if let (Some(a), Some(b)) = (sim.0.unit(p), sim.0.unit(*target)) {
                    sig.push(Signal::RangedHit(combat_core::distance(a.pos, b.pos)));
                }
            }
            Event::Heal { source, target, .. } if *source == p && Some(*target) == colleague => {
                sig.push(Signal::HealedColleague)
            }
            Event::TelegraphStarted {
                id, source, spell, ..
            } if is_dummy(source) && *spell == ZONE_SPELL => {
                run.zones.push(*id);
            }
            Event::TelegraphResolved { id, landed } if run.zones.contains(id) => {
                run.zones.retain(|z| z != id);
                zone_landed |= *landed;
            }
            Event::Looted { looter, corpse, .. } if *looter == p && is_dummy(corpse) => {
                sig.push(Signal::LootedDummy)
            }
            _ => {}
        }
    }
    if zone_landed && !zone_hit {
        sig.push(Signal::DodgedZone);
    }
    for s in sig {
        if run.tut.feed(s) {
            run.flash = None;
        }
    }
}

/// The new classes' openers are debuffs, not damage: say so on the role tip.
fn class_tip(class: &str, spell2: &(String, String)) -> Option<String> {
    let (name, key) = spell2;
    match class {
        "qa" => Some(format!(
            "File a {name} ({key}) first: every report makes the target take 6% more damage, up to three."
        )),
        "data_scientist" => Some(format!(
            "Stack {name} ({key}) up to three times first; it keeps ticking while you cast."
        )),
        _ => None,
    }
}

fn class_key(sim: &CombatSim, p: EntityId) -> String {
    sim.0.class_of(p).map(|c| c.key.clone()).unwrap_or_default()
}

fn talents(sim: &CombatSim, p: EntityId) -> combat_core::TalentPicks {
    sim.0
        .unit(p)
        .map_or([0; combat_core::TALENT_ROWS], |u| u.talents)
}

/// The dummy outlives the lessons it is needed for, and the colleague is hurt when a healer
/// needs someone to heal. Offline only, so touching the sim directly is safe.
fn keep_props_useful(mut onb: ResMut<Onboarding>, mut sim: ResMut<CombatSim>, time: Res<Time>) {
    let Some(run) = onb.run.as_mut() else { return };
    let Some(step) = run.tut.current() else {
        return;
    };
    let before_loot =
        STEPS.iter().position(|s| *s == step) < STEPS.iter().position(|s| *s == Step::Loot);
    if before_loot
        && let Some(d) = find_unit(&sim, DUMMY)
        && let Some(u) = sim.0.unit(d).filter(|u| u.alive)
        && u.hp * 100 < u.max_hp * DUMMY_FLOOR_PCT
    {
        let max = u.max_hp;
        sim.0.debug_set_hp(d, max);
        run.flash = Some((
            "Session timed out at 70% complete. Progress lost. Please restart the module.".into(),
            time.elapsed_secs(),
        ));
    }
    if step == Step::RoleTip
        && run.tut.role == Role::Healer
        && let Some(c) = find_unit(&sim, COLLEAGUE)
        && let Some(u) = sim.0.unit(c).filter(|u| u.alive && u.hp == u.max_hp)
    {
        let hp = u.max_hp * COLLEAGUE_HP_PCT / 100;
        sim.0.debug_set_hp(c, hp);
    }
}

fn ctx(sim: &CombatSim, p: EntityId, binds: &Keybinds, touch: bool) -> Ctx {
    let k = |a| binds.label(a);
    let slot = |i: u8| {
        let c = sim.0.content();
        let name = sim
            .0
            .unit(p)
            .and_then(|u| c.bar_for_unit(&u.key).get(usize::from(i)).copied())
            .and_then(|id| c.spell(id))
            .map_or_else(|| format!("slot {}", i + 1), |s| s.name.clone());
        (name, k(Action::Slot(i)))
    };
    let role = sim.0.class_of(p).map(|c| c.role);
    let role_spell = match role {
        Some(Role::Tank) => bar_spell(sim, p, |s| matches!(s.effect, Effect::Absorb { .. })),
        Some(Role::Healer) => bar_spell(sim, p, |s| {
            matches!(s.effect, Effect::Heal { .. }) && s.target == SpellTarget::Ally
        }),
        _ => None,
    }
    .map_or_else(|| slot(0), |(i, name, _)| (name, k(Action::Slot(i))));
    Ctx {
        touch,
        move_keys: format!(
            "{}{}{}{}",
            k(Action::Forward),
            k(Action::TurnLeft),
            k(Action::Backward),
            k(Action::TurnRight)
        ),
        target: k(Action::TargetNext),
        auto: k(Action::AutoAttack),
        interact: k(Action::Interact),
        map: k(Action::Map),
        sheet: k(Action::CharacterSheet),
        career: k(Action::Career),
        scooter: k(Action::Scooter),
        spell1: slot(0),
        spell2: slot(1),
        interrupt: bar_spell(sim, p, is_interrupt)
            .map(|(i, name, lvl)| (name, k(Action::Slot(i)), lvl)),
        role_spell,
        taunt: bar_spell(sim, p, |s| matches!(s.effect, Effect::Taunt))
            .map(|(_, name, lvl)| (name, lvl)),
        class_tip: class_tip(&class_key(sim, p), &slot(1)),
    }
}

/// The HUD element a step talks about.
fn spot_for(sim: &CombatSim, p: EntityId, t: &Tutorial, touch: bool) -> Option<Spot> {
    let role_slot = |want: &dyn Fn(&combat_core::SpellDef) -> bool| {
        bar_spell(sim, p, want).map(|(i, ..)| Spot::Slot(i))
    };
    match t.current()? {
        Step::Accept | Step::Loot | Step::Handoff if touch => Some(Spot::Touch(Action::Interact)),
        Step::AutoAttack => Some(Spot::AutoAttack),
        Step::Spell1 => Some(Spot::Slot(0)),
        Step::Spell2 => Some(Spot::Slot(1)),
        Step::Interrupt if t.can_interrupt => role_slot(&is_interrupt),
        Step::Interrupt => Some(Spot::TargetFrame),
        Step::RoleTip => match t.role {
            Role::Tank => role_slot(&|s| matches!(s.effect, Effect::Absorb { .. })),
            Role::Healer => role_slot(&|s| {
                matches!(s.effect, Effect::Heal { .. }) && s.target == SpellTarget::Ally
            }),
            _ => Some(Spot::TargetFrame),
        },
        Step::TurnIn => Some(Spot::Tracker),
        Step::Windows if touch => Some(Spot::Touch(Action::Map)),
        Step::Windows => Some(Spot::Minimap),
        Step::Scooter if touch => Some(Spot::Touch(Action::Scooter)),
        Step::Scooter => Some(Spot::Mount),
        _ => None,
    }
}

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                // Under the key hint and the window strip that hangs from the top edge.
                top: px(48),
                left: percent(50),
                margin: UiRect::left(px(-PANEL_W / 2.0)),
                width: px(PANEL_W),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                padding: UiRect::all(px(10)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            panel_bg(),
            GlobalZIndex(40),
        ))
        .with_children(|p| {
            p.spawn((StepText, label("", 11.0), TextColor(DIM)));
            p.spawn((TitleText, label("", 16.0), TextColor(ACCENT)));
            p.spawn((
                HowText,
                label("", 13.0),
                TextColor(Color::srgb(0.92, 0.95, 1.0)),
            ));
            p.spawn((QuipText, label("", 12.0), TextColor(DIM)));
            p.spawn(Node {
                column_gap: px(3),
                margin: UiRect::top(px(2)),
                ..default()
            })
            .with_children(|row| {
                for i in 0..STEPS.len() {
                    row.spawn((
                        ProgressCell(i),
                        Node {
                            flex_grow: 1.0,
                            height: px(4),
                            border_radius: BorderRadius::all(px(2)),
                            ..default()
                        },
                        BackgroundColor(KEY_TOP),
                    ));
                }
            });
            p.spawn(Node {
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Button,
                    SkipButton,
                    Node {
                        padding: UiRect::axes(px(10), px(3)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    vgrad(KEY_TOP_HI, KEY_TOP),
                    children![label("Skip onboarding (I've read the docs)", 12.0)],
                ));
            });
        });
    commands.spawn((
        ReplayMenu,
        Node {
            position_type: PositionType::Absolute,
            // Same row as "Change role", to its right (that row's hint text sits below it).
            top: percent(81),
            width: percent(100),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        GlobalZIndex(51),
        children![(
            Button,
            ReplayButton,
            Node {
                margin: UiRect::left(px(390)),
                padding: UiRect::axes(px(14), px(5)),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BorderColor::all(KEY_RIM),
            vgrad(KEY_TOP_HI, KEY_TOP),
            children![label("Replay onboarding", 16.0)],
        )],
    ));
}

fn panel(
    onb: Res<Onboarding>,
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    binds: Res<Keybinds>,
    touch: Res<TouchMode>,
    time: Res<Time>,
    mut root: Single<&mut Node, With<PanelRoot>>,
    mut texts: ParamSet<(
        Single<&mut Text, With<StepText>>,
        Single<&mut Text, With<TitleText>>,
        Single<&mut Text, With<HowText>>,
        Single<(&mut Text, &mut TextColor), With<QuipText>>,
    )>,
    mut cells: Query<(&ProgressCell, &mut BackgroundColor)>,
) {
    let show = onb.run.is_some() && flow.zone == STARTER && !flow.input_locked();
    let want = if show { Display::Flex } else { Display::None };
    if root.display != want {
        root.display = want;
    }
    let (Some(run), Some(p)) = (onb.run.as_ref(), sim.0.player()) else {
        return;
    };
    let Some(copy) = steps::copy(&run.tut, &ctx(&sim, p, &binds, touch.active)) else {
        return;
    };
    set_text(
        &mut texts.p0(),
        &format!(
            "// onboarding.md   step {}/{}   completion is tracked",
            run.tut.step + 1,
            STEPS.len()
        ),
    );
    set_text(&mut texts.p1(), &format!("> {}", copy.title));
    set_text(&mut texts.p2(), &copy.how);
    let now = time.elapsed_secs();
    let flash = run
        .flash
        .as_ref()
        .filter(|(_, t)| now - t < FLASH_SECS)
        .map(|(s, _)| s.as_str());
    let mut quip = texts.p3();
    let (text, color) = &mut *quip;
    set_text(text, flash.unwrap_or(copy.quip));
    let c = if flash.is_some() { ACCENT_WARM } else { DIM };
    if color.0 != c {
        color.0 = c;
    }
    for (cell, mut bg) in &mut cells {
        let c = if cell.0 < run.tut.step {
            ACCENT
        } else if cell.0 == run.tut.step {
            ACCENT_WARM
        } else {
            crate::hud::KEY_TOP
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

/// Pulsing outline on the element the current step talks about; nothing else gets one.
fn highlight(
    mut commands: Commands,
    onb: Res<Onboarding>,
    sim: Res<CombatSim>,
    flow: Res<GameFlow>,
    touch: Res<TouchMode>,
    time: Res<Time>,
    mut spots: Query<(Entity, &Spot, Option<&mut Outline>)>,
) {
    let want = match (onb.run.as_ref(), sim.0.player()) {
        (Some(run), Some(p)) if flow.zone == STARTER => spot_for(&sim, p, &run.tut, touch.active),
        _ => None,
    };
    let pulse = 0.55 + 0.45 * (time.elapsed_secs() * 5.0).sin();
    let color = ACCENT_WARM.with_alpha(pulse);
    for (e, spot, outline) in &mut spots {
        match (Some(*spot) == want, outline) {
            (true, Some(mut o)) => o.color = color,
            (true, None) => {
                commands.entity(e).insert(Outline::new(px(3), px(2), color));
            }
            (false, Some(_)) => {
                commands.entity(e).remove::<Outline>();
            }
            (false, None) => {}
        }
    }
}

/// Esc menu "Replay onboarding": back to the arrival spot with fresh props, step one.
fn replay_menu(
    mut paused: ResMut<Paused>,
    lobby: Res<Lobby>,
    mode: Res<GameMode>,
    mut onb: ResMut<Onboarding>,
    mut menu: Single<&mut Node, With<ReplayMenu>>,
    button: Query<&Interaction, (Changed<Interaction>, With<ReplayButton>)>,
    mut enter: MessageWriter<EnterZone>,
) {
    let show = paused.0 && !lobby.open && !mode.is_online();
    let want = if show { Display::Flex } else { Display::None };
    if menu.display != want {
        menu.display = want;
    }
    if show && button.iter().any(|i| *i == Interaction::Pressed) {
        onb.run = None;
        onb.want = true;
        // Reloading P3 respawns the dummy and puts the player at the arrival spot.
        enter.write(EnterZone {
            zone: STARTER.into(),
            from: None,
        });
        paused.0 = false;
    }
}

/// Night spawns at the player's level would ambush a level-1 tutorial after 18:00.
fn hold_night(onb: Res<Onboarding>, mut sim: ResMut<CombatSim>) {
    let hold = onb.run.is_some() || onb.want;
    if sim.0.is_night_held() != hold {
        sim.0.set_hold_night(hold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hire_starts_in_car_park_p3() {
        let mut onb = Onboarding::default();
        onb.class_loaded(true);
        assert!(sends_new_hire_to_p3(&onb, "campus", false));
        // Not during a fade, never twice, and not once they are there.
        assert!(!sends_new_hire_to_p3(&onb, "campus", true));
        assert!(!sends_new_hire_to_p3(&onb, STARTER, false));
        onb.redirected = true;
        assert!(!sends_new_hire_to_p3(&onb, "campus", false));
        // A returning player (save says onboarding is done) stays where they are.
        onb.class_loaded(false);
        assert!(!sends_new_hire_to_p3(&onb, "campus", false));
        assert_eq!(STARTER, "parking_p3");
    }
}
