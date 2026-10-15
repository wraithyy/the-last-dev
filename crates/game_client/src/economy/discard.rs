//! Throwing things out of the bag: the confirm popup, the "rm -rf ./junk" header button, the
//! touch tooltip's Delete button and the chat line that reports a sweep or a bulk sale. The sim
//! decides what may be destroyed; this only asks and reports.

use super::goods::{self, DOWNGRADE_RED, JUNK_GREY};
use super::{EconomyUi, Inspect, Tooltip};
use crate::combat::{PendingCommands, SimRead, SimReady, SimView};
use crate::copy::kudos;
use crate::hud::{ACCENT, DIM, LogLineMsg, PANEL_EDGE, drop_shadow, label, panel_bg, set_text};
use bevy::prelude::*;
use combat_core::{Command, Goods};

/// The item the popup asks about.
#[derive(Resource, Default)]
pub struct DiscardUi {
    pub confirm: Option<String>,
}

/// What a bulk action should report once the sim's answer arrives in the bag.
#[derive(Clone, Copy)]
pub enum Sweep {
    Delete { bag_len: usize },
    Sell { bag_len: usize, sp: u32 },
}

#[derive(Resource, Default)]
pub struct SweepWatch(pub Option<(Sweep, f32)>);

/// Seconds to wait for the sim before giving up on a report (online round trips).
const WATCH_SECS: f32 = 3.0;

#[derive(Component)]
pub struct JunkButton;
#[derive(Component)]
struct Popup;
#[derive(Component)]
struct PopupText;
#[derive(Component)]
struct PopupYes;
#[derive(Component)]
struct PopupNo;
#[derive(Component)]
struct DeleteTip;

pub struct DiscardPlugin;

impl Plugin for DiscardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiscardUi>()
            .init_resource::<SweepWatch>()
            .add_systems(
                Startup,
                (spawn_popup, spawn_delete_tip.after(super::spawn_tooltip)),
            )
            .add_systems(
                Update,
                (popup, junk_button, delete_tip, report, debug).in_set(SimReady),
            );
    }
}

pub fn is_junk(sim: &dyn SimRead, name: &str) -> bool {
    matches!(sim.content().economy.goods(name), Some(Goods::Junk(_)))
}

/// Junk goes at once, anything else waits for the popup. The last copy is the one deleted, so a
/// stack loses one unit at a time.
pub fn request(ui: &mut DiscardUi, pending: &mut PendingCommands, sim: &dyn SimRead, name: &str) {
    if is_junk(sim, name) {
        send(pending, sim, name);
    } else {
        ui.confirm = Some(name.to_string());
    }
}

fn send(pending: &mut PendingCommands, sim: &dyn SimRead, name: &str) {
    let slot = sim
        .player()
        .and_then(|p| sim.unit(p))
        .and_then(|u| u.bag.iter().rposition(|n| n == name))
        .and_then(|k| u16::try_from(k).ok());
    if let Some(slot) = slot {
        pending.0.push(Command::Discard { slot });
    }
}

fn spawn_popup(mut commands: Commands) {
    let button = |text: &'static str, color: Color| {
        (
            Button,
            Node {
                padding: UiRect::axes(px(10), px(4)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BorderColor::all(color),
            children![(label(text, 13.0), TextColor(color))],
        )
    };
    commands
        .spawn((
            Popup,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(38),
                width: px(320),
                margin: UiRect::left(px(-160)),
                padding: UiRect::all(px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(DOWNGRADE_RED.with_alpha(0.8)),
            panel_bg(),
            drop_shadow(),
            GlobalZIndex(70),
        ))
        .with_children(|c| {
            c.spawn((PopupText, label("", 13.0), TextColor(Color::WHITE)));
            c.spawn(Node {
                column_gap: px(10),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|row| {
                row.spawn((PopupNo, button("[Cancel]", DIM)));
                row.spawn((PopupYes, button("[Delete]", DOWNGRADE_RED)));
            });
        });
}

fn popup(
    mut ui: ResMut<DiscardUi>,
    shell: Res<EconomyUi>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    mut window: Single<&mut Node, With<Popup>>,
    mut text: Single<&mut Text, With<PopupText>>,
    yes: Query<&Interaction, (With<PopupYes>, Changed<Interaction>)>,
    no: Query<&Interaction, (With<PopupNo>, Changed<Interaction>)>,
) {
    if !shell.sheet {
        ui.confirm = None;
    }
    if no.iter().any(|i| *i == Interaction::Pressed) {
        ui.confirm = None;
    }
    if yes.iter().any(|i| *i == Interaction::Pressed)
        && let Some(name) = ui.confirm.take()
    {
        send(&mut pending, &*sim, &name);
    }
    let Some(name) = &ui.confirm else {
        window.display = Display::None;
        return;
    };
    window.display = Display::Flex;
    set_text(
        &mut text,
        &format!("Delete {name}? This cannot be undone. Like prod."),
    );
}

fn junk_count(sim: &dyn SimRead) -> usize {
    sim.player()
        .and_then(|p| sim.unit(p))
        .map_or(0, |u| u.bag.iter().filter(|n| is_junk(sim, n)).count())
}

/// "rm -rf ./junk" in the bag header; dim while there is no junk.
fn junk_button(
    sim: SimView,
    shell: Res<EconomyUi>,
    mut pending: ResMut<PendingCommands>,
    mut watch: ResMut<SweepWatch>,
    mut lines: MessageWriter<LogLineMsg>,
    buttons: Query<(&Interaction, &Children), With<JunkButton>>,
    mut colors: Query<&mut TextColor>,
) {
    if !shell.sheet {
        return;
    }
    let junk = junk_count(&*sim);
    for (i, kids) in &buttons {
        for k in kids.iter() {
            if let Ok(mut c) = colors.get_mut(k) {
                c.0 = if junk == 0 { JUNK_GREY } else { ACCENT };
            }
        }
        if *i != Interaction::Pressed {
            continue;
        }
        if junk == 0 {
            lines.write(LogLineMsg(
                "rm: ./junk: No such file or directory".into(),
                JUNK_GREY,
            ));
            continue;
        }
        let bag_len = sim
            .player()
            .and_then(|p| sim.unit(p))
            .map_or(0, |u| u.bag.len());
        watch.0 = Some((Sweep::Delete { bag_len }, WATCH_SECS));
        pending.0.push(Command::DiscardJunk);
    }
}

fn spawn_delete_tip(mut commands: Commands, tooltip: Single<Entity, With<Tooltip>>) {
    commands.entity(*tooltip).with_child((
        DeleteTip,
        Button,
        Node {
            padding: UiRect::axes(px(10), px(6)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(DOWNGRADE_RED),
        BackgroundColor(Color::srgb(0.04, 0.045, 0.07)),
        children![(label("[Delete]", 13.0), TextColor(DOWNGRADE_RED))],
    ));
}

/// On touch the pinned tooltip of a bag item carries a Delete button.
fn delete_tip(
    shell: Res<EconomyUi>,
    touch: Res<crate::touch::TouchMode>,
    mut inspect: ResMut<Inspect>,
    mut ui: ResMut<DiscardUi>,
    sim: SimView,
    mut pending: ResMut<PendingCommands>,
    mut button: Single<(&mut Node, &Interaction), With<DeleteTip>>,
) {
    let name = inspect.name.clone().filter(|n| {
        touch.active
            && shell.sheet
            && shell.shop.is_none()
            && !inspect.buying
            && !inspect.loot
            && sim
                .player()
                .and_then(|p| sim.unit(p))
                .is_some_and(|u| u.bag.contains(n))
    });
    let (node, interaction) = &mut *button;
    node.display = if name.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if let Some(name) = name
        && **interaction == Interaction::Pressed
    {
        inspect.name = None;
        request(&mut ui, &mut pending, &*sim, &name);
    }
}

/// Chat line once the bag reflects a sweep or a bulk sale.
fn report(
    time: Res<Time>,
    sim: SimView,
    mut watch: ResMut<SweepWatch>,
    mut lines: MessageWriter<LogLineMsg>,
) {
    let Some((sweep, left)) = watch.0.as_mut() else {
        return;
    };
    *left -= time.delta_secs();
    let expired = *left <= 0.0;
    let Some(me) = sim.player().and_then(|p| sim.unit(p)) else {
        return;
    };
    match *sweep {
        Sweep::Delete { bag_len } if me.bag.len() < bag_len => {
            let n = bag_len - me.bag.len();
            lines.write(LogLineMsg(
                format!("rm -rf ./junk: {n} item{} deleted. No backups.", plural(n)),
                goods::PERK_GOLD,
            ));
            watch.0 = None;
        }
        Sweep::Sell { bag_len, sp } if me.bag.len() < bag_len => {
            let n = bag_len - me.bag.len();
            lines.write(LogLineMsg(
                format!(
                    "Sold {n} junk item{} for {}.",
                    plural(n),
                    kudos(me.sp.saturating_sub(sp))
                ),
                goods::SP_GOLD,
            ));
            watch.0 = None;
        }
        _ if expired => watch.0 = None,
        _ => {}
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Header row of the bag: the caption on the left, the junk button on the right.
pub fn bag_header(c: &mut ChildSpawnerCommands, caption: &str) {
    c.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|h| {
        h.spawn((label(caption, 12.0), TextColor(DIM)));
        h.spawn((
            JunkButton,
            Button,
            Node {
                padding: UiRect::axes(px(6), px(2)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            children![(label("rm -rf ./junk", 12.0), TextColor(ACCENT))],
        ));
    });
}

/// `WOW_DISCARD_POPUP=<item>@<secs>` opens the delete popup; `WOW_JUNK_SWEEP=<secs>` presses
/// "rm -rf ./junk" (screenshots have no pointer).
fn debug(
    time: Res<Time>,
    sim: SimView,
    mut ui: ResMut<DiscardUi>,
    mut pending: ResMut<PendingCommands>,
    mut watch: ResMut<SweepWatch>,
    mut done: Local<(bool, bool)>,
) {
    let now = time.elapsed_secs();
    if let Some((item, at)) = std::env::var("WOW_DISCARD_POPUP").ok().and_then(|v| {
        v.rsplit_once('@')
            .map(|(i, t)| (i.to_string(), t.parse::<f32>().ok()))
    }) && let Some(at) = at
        && !done.0
        && now >= at
    {
        done.0 = true;
        ui.confirm = Some(item);
    }
    let at = std::env::var("WOW_JUNK_SWEEP")
        .ok()
        .and_then(|v| v.parse::<f32>().ok());
    if let Some(at) = at
        && !done.1
        && now >= at
        && let Some(me) = sim.player().and_then(|p| sim.unit(p))
    {
        done.1 = true;
        let bag_len = me.bag.len();
        watch.0 = Some((Sweep::Delete { bag_len }, WATCH_SECS));
        pending.0.push(Command::DiscardJunk);
    }
}
