//! Quests in Jira clothes: the giver dialog (a ticket with Accept / Complete and a "choose one"
//! gear reward), the WoW-style objective tracker on the right ("MeatProxies terminated 5/10"),
//! progress toasts, the "Ticket closed" banner and the `~/jira` log (J) with track toggles.
//! Everything is read from the sim (or the online mirror) each frame; widgets are rebuilt only
//! when the shown state changes.

use super::scroll::{ScrollList, key_of};
use super::{
    ACCENT, BAR_BG, Banner, DIM, IconCache, PANEL_EDGE, asset_key, drop_shadow, label, panel_bg,
    put, set_text,
};
use crate::combat::{CombatEvent, PendingCommands, SimRead, SimReady, SimView, Talked};
use crate::copy;
use crate::economy::goods;
use crate::game::{GameFlow, zone_label};
use crate::input::{Paused, QuestWindows, TrophiesOpen};
use crate::player::Player;
use crate::touch::TouchMode;
use bevy::prelude::*;
use combat_core::{Command, Content, Event, Objective, QuestDef, QuestStatus};

pub const GOLD: Color = Color::srgb(1.0, 0.82, 0.3);
/// Over the heads of story quest givers and in the main-quest labels.
pub const MAIN_MARK: Color = Color::srgb(1.0, 0.72, 0.1);
/// Over the heads of side quest givers.
pub const SIDE_MARK: Color = Color::srgb(0.45, 0.8, 1.0);
pub const GREY_MARK: Color = Color::srgb(0.62, 0.64, 0.7);
const DONE_GREEN: Color = Color::srgb(0.4, 1.0, 0.5);
const DONE_GREY: Color = Color::srgb(0.5, 0.53, 0.58);
const FAILED_RED: Color = Color::srgb(1.0, 0.4, 0.35);
const WINDOW_W: f32 = 460.0;
pub(crate) const TRACKER_W: f32 = 290.0;
/// Quests the tracker shows at most; touch screens have less room under the minimap.
const TRACKER_MAX: usize = 6;
const TRACKER_MAX_TOUCH: usize = 3;
const TOAST_SECS: f32 = 2.5;
pub(crate) const TRACKER_TOP: f32 = super::BELOW_MINIMAP + 34.0;
/// Height kept free for the chat / combat log / meter windows docked bottom right.
const DOCK_RESERVE: f32 = 230.0;
/// Touch layout: under the player frame on the left, above the move stick.
pub(crate) const TOUCH_TOP: f32 = 120.0;
const TOUCH_RESERVE: f32 = 340.0;
/// Rough height of one tracker text line, for the fit check.
const LINE_PX: f32 = 16.0;

/// "SPRINT-101": the quest's position in content, so ids are stable across runs.
pub fn ticket_id(content: &Content, id: &str) -> String {
    let n = content.quests.iter().position(|q| q.id == id).unwrap_or(0);
    format!("SPRINT-{}", 101 + n)
}

pub fn unit_name(content: &Content, key: &str) -> String {
    content
        .unit_def(key)
        .map_or_else(|| key.to_string(), |u| u.name.clone())
}

/// English plural of a display name: "MeatProxy" -> "MeatProxies", "Agile Coach" -> "Agile
/// Coaches". Names ending in a bracket or an "s" stay as they are.
pub fn plural(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let consonant_y = lower.ends_with('y')
        && !lower.ends_with("ay")
        && !lower.ends_with("ey")
        && !lower.ends_with("oy");
    if name.ends_with(')') || lower.ends_with('s') {
        name.to_string()
    } else if consonant_y {
        format!("{}ies", &name[..name.len() - 1])
    } else if lower.ends_with("ch") || lower.ends_with("sh") || lower.ends_with('x') {
        format!("{name}es")
    } else {
        format!("{name}s")
    }
}

/// What an objective asks for, without the count.
pub fn objective_label(content: &Content, o: &Objective) -> String {
    match o {
        Objective::Kill(k, n) => {
            let name = unit_name(content, k);
            let who = if *n > 1 { plural(&name) } else { name };
            format!("{who} terminated")
        }
        Objective::Loot(item, n) if *n > 1 => plural(item),
        Objective::Loot(item, _) => item.clone(),
        Objective::Talk(k) => format!("Talk to {}", unit_name(content, k)),
        Objective::ClearZone(z) => format!("Clear {}", zone_label(content, z)),
        Objective::KillClean(k, n) => {
            let name = unit_name(content, k);
            let who = if *n > 1 { plural(&name) } else { name };
            format!("{who} terminated, no telegraph hits")
        }
        Objective::Wait { label, .. } | Objective::Defend { label, .. } => label.clone(),
        Objective::Interact(k, _) => {
            let name = content.prop(k).map_or(k.as_str(), |p| p.name.as_str());
            format!("Use {name}")
        }
        Objective::Collect(tag, n) => format!("{} collected", counted(&tag_name(content, tag), *n)),
        Objective::Inspect(tag, n) => {
            format!("{} documented", counted(&tag_name(content, tag), *n))
        }
        Objective::Sequence { label, .. } | Objective::Escort { label, .. } => label.clone(),
        Objective::UseOn { item, target, n } => {
            let who = content
                .unit_def(target)
                .map(|u| u.name.clone())
                .unwrap_or_else(|| tag_name(content, target));
            format!("Use {item} on {}", counted(&who, *n))
        }
        Objective::Deliver { item, npc, .. } => {
            format!("Deliver {item} to {}", unit_name(content, npc))
        }
    }
}

/// The enemy F hands a quest item to (`UseOn` on a hostile key): the target when it
/// qualifies, else the nearest wanted one within `reach` of `from`.
pub fn use_on_target(
    sim: &dyn SimRead,
    target: Option<combat_core::EntityId>,
    from: combat_core::Vec3,
    reach: f32,
) -> Option<combat_core::EntityId> {
    let content = sim.content();
    let wanted: Vec<&str> = sim
        .quests()
        .active
        .iter()
        .filter_map(|a| content.quest(&a.id).map(|d| (d, a)))
        .flat_map(|(d, a)| d.objectives.iter().zip(&a.counts))
        .filter_map(|(o, c)| match o {
            Objective::UseOn { target, .. } if *c < o.need() => Some(target.as_str()),
            _ => None,
        })
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let ok = |u: &combat_core::UnitState| {
        u.alive
            && u.hostile
            && wanted.contains(&u.key.as_str())
            && combat_core::in_range(u.pos, from, reach)
    };
    if let Some(t) = target.and_then(|t| sim.unit(t)).filter(|u| ok(u)) {
        return Some(t.id);
    }
    sim.units()
        .filter(|u| ok(u))
        .min_by(|a, b| {
            combat_core::distance(a.pos, from).total_cmp(&combat_core::distance(b.pos, from))
        })
        .map(|u| u.id)
}

/// Display name of a quest prop group: the first prop whose key or tag is `tag`.
fn tag_name(content: &Content, tag: &str) -> String {
    content
        .world
        .props
        .iter()
        .find(|p| p.tag == tag || p.key == tag)
        .map_or_else(|| tag.to_string(), |p| p.name.clone())
}

/// `name` in the plural when `n` > 1, pluralising the head noun of a described thing:
/// "Sticky Notes with a Password", "Gantt Charts (Pure Fiction)".
fn counted(name: &str, n: u32) -> String {
    if n <= 1 {
        return name.to_string();
    }
    let cut = [" with ", " Under ", " Showing ", " Taped ", " ("]
        .iter()
        .filter_map(|c| name.find(c))
        .min();
    match cut {
        Some(i) => format!("{}{}", plural(&name[..i]), &name[i..]),
        None => plural(name),
    }
}

/// A fixed-width text bar for slow waits: "[====......] 40%".
fn wait_bar(have: u32, need: u32) -> String {
    const CELLS: u32 = 20;
    let need = need.max(1);
    let filled = (have.min(need) * CELLS / need) as usize;
    format!(
        "[{}{}] {}%",
        "=".repeat(filled),
        ".".repeat(CELLS as usize - filled),
        have.min(need) * 100 / need
    )
}

/// One tracker/toast line: "MeatProxies terminated 5/10", "Access Badges 2/5"; talk and clear
/// objectives read as a plain task ("Talk to Facilities Guy") until done.
pub fn objective_line(content: &Content, o: &Objective, have: u32, need: u32) -> String {
    let what = objective_label(content, o);
    match o {
        Objective::Kill(..) | Objective::Loot(..) | Objective::KillClean(..) => {
            format!("{what} {have}/{need}")
        }
        Objective::Interact(..) | Objective::UseOn { .. } if need > 1 => {
            format!("{what} {have}/{need}")
        }
        Objective::Interact(..) | Objective::UseOn { .. } => what,
        Objective::Collect(..) | Objective::Inspect(..) => format!("{what} {have}/{need}"),
        // The next step is the hint: the order is the puzzle, not finding the machines.
        Objective::Sequence { props, .. } => match props.get(have as usize) {
            Some(next) if have < need => {
                let name = content
                    .prop(next)
                    .map_or(next.as_str(), |p| p.name.as_str());
                format!("{what} {have}/{need} \u{2014} next: {name}")
            }
            _ => format!("{what} {have}/{need}"),
        },
        Objective::Wait { .. } | Objective::Defend { .. } | Objective::Escort { .. } => {
            format!("{what} {}", wait_bar(have, need))
        }
        Objective::Talk(_) | Objective::ClearZone(_) | Objective::Deliver { .. } => what,
    }
}

/// Kept for the toast and tests: the same text as the tracker line.
pub fn progress_line(content: &Content, o: &Objective, count: u32, need: u32) -> String {
    objective_line(content, o, count, need)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    Pending,
    Done,
    /// "Ready to close" in gold.
    Ready,
}

/// The tracker body of one quest: every objective with its state, then the turn-in line once
/// all are done.
pub fn tracker_lines(content: &Content, def: &QuestDef, counts: &[u32]) -> Vec<(String, Line)> {
    let mut out: Vec<(String, Line)> = def
        .objectives
        .iter()
        .enumerate()
        .map(|(k, o)| {
            let need = o.need();
            let have = counts.get(k).copied().unwrap_or(0).min(need);
            let state = if have >= need {
                Line::Done
            } else {
                Line::Pending
            };
            (objective_line(content, o, have, need), state)
        })
        .collect();
    if out.iter().all(|(_, l)| *l == Line::Done) {
        out.push((
            format!(
                "Ready to close \u{2014} {} {}",
                // Turn-ins stand where the work ends: you go on to them, not back.
                if def.turn_in.is_some() {
                    "report to"
                } else {
                    "return to"
                },
                unit_name(content, &def.ender().npc)
            ),
            Line::Ready,
        ));
    }
    out
}

/// The NPC's quests the player can act on in `zone`, content order: pick-ups at their giver,
/// open and finished work at whoever takes it (the giver, or a breadcrumb's target).
pub fn giver_quests(sim: &dyn SimRead, npc_key: &str, zone: &str) -> Vec<(String, QuestStatus)> {
    sim.content()
        .quests
        .iter()
        .map(|q| (q, sim.quest_status(&q.id)))
        .filter(|(q, s)| match s {
            QuestStatus::Available => q.giver.npc == npc_key && q.giver.zone == zone,
            QuestStatus::Active | QuestStatus::Ready => {
                q.ender().npc == npc_key && q.ender().zone == zone
            }
            _ => false,
        })
        .map(|(q, s)| (q.id.clone(), s))
        .collect()
}

/// Gold marks are story (main) quests, azure ones side quests; the grey "?" for pending work
/// is the same for both. "?" to turn in, "!" to pick up. A Company Calendar chain's first quest
/// gets no "!": the calendar board carries one for all of them.
pub fn giver_mark(sim: &dyn SimRead, npc_key: &str, zone: &str) -> Option<(&'static str, Color)> {
    let mut qs = giver_quests(sim, npc_key, zone);
    let content = sim.content();
    let chains = &content.calendar.chains;
    qs.retain(|(id, s)| {
        *s != QuestStatus::Available || !chains.iter().any(|c| c.quests.first() == Some(id))
    });
    let has = |s: QuestStatus| qs.iter().any(|(_, q)| *q == s);
    let tint = |s: QuestStatus| {
        let main = qs
            .iter()
            .any(|(id, q)| *q == s && content.quest(id).is_some_and(|d| d.main));
        if main { MAIN_MARK } else { SIDE_MARK }
    };
    if has(QuestStatus::Ready) {
        Some(("?", tint(QuestStatus::Ready)))
    } else if has(QuestStatus::Available) {
        Some(("!", tint(QuestStatus::Available)))
    } else if has(QuestStatus::Active) {
        Some(("?", GREY_MARK))
    } else if too_low_for_quest(sim, npc_key, zone) {
        Some(("!", GREY_MARK))
    } else {
        None
    }
}

/// How many levels short of a ticket the player may be and still see its grey "!".
const GREY_BANG_LEVELS: u8 = 3;

/// The NPC gives a ticket whose only missing gate is the player's level, a few levels away.
fn too_low_for_quest(sim: &dyn SimRead, npc_key: &str, zone: &str) -> bool {
    let Some(level) = sim.player().and_then(|p| sim.unit(p)).map(|u| u.level) else {
        return false;
    };
    let done = &sim.quests().completed;
    sim.content().quests.iter().any(|q| {
        q.giver.npc == npc_key
            && q.giver.zone == zone
            && q.level_min > level
            && q.level_min <= level.saturating_add(GREY_BANG_LEVELS)
            && q.requires.as_ref().is_none_or(|r| done.contains(r))
            && sim.quest_status(&q.id) == QuestStatus::Locked
    })
}

/// Which quests the tracker follows: auto-tracked on accept, untracked from the log (J).
#[derive(Resource, Default)]
pub struct QuestTrack {
    pub untracked: Vec<String>,
    pub collapsed: bool,
    /// Quest the log (J) opens on, from a tracker title click.
    pub focus: Option<String>,
    /// Quest the waypoint arrow follows (picked in the log or by a tracker title click);
    /// the first tracked quest when unset or no longer open.
    pub waypoint: Option<String>,
    /// Timed deliveries: (quest, `Time::elapsed_secs` when it runs out), started on accept so
    /// the countdown works the same offline and online.
    pub deadlines: Vec<(String, f32)>,
}

impl QuestTrack {
    pub fn is_tracked(&self, id: &str) -> bool {
        !self.untracked.iter().any(|u| u == id)
    }
}

/// Tracked open quests, main quests first, each group in accept order.
pub fn tracked<'a>(sim: &'a dyn SimRead, track: &QuestTrack) -> Vec<&'a combat_core::ActiveQuest> {
    let content = sim.content();
    let mut out: Vec<_> = sim
        .quests()
        .active
        .iter()
        .filter(|q| track.is_tracked(&q.id))
        .collect();
    // Stable: accept order survives inside each group.
    out.sort_by_key(|q| !content.quest(&q.id).is_some_and(|d| d.main));
    out
}

/// "MAIN QUEST" in gold or "SIDE QUEST" in azure, for dialogs and the log.
fn kind_tag(def: &QuestDef) -> (&'static str, Color) {
    if def.main {
        ("MAIN QUEST", MAIN_MARK)
    } else {
        ("SIDE QUEST", SIDE_MARK)
    }
}

/// Unit keys one objective works on (kill targets, droppers of wanted items, the boss of a zone
/// to clear, NPCs to talk to); none for work that happens at props.
pub fn objective_unit_keys(content: &Content, o: &Objective) -> Vec<String> {
    match o {
        Objective::Kill(u, _)
        | Objective::Talk(u)
        | Objective::KillClean(u, _)
        | Objective::Deliver { npc: u, .. }
        | Objective::UseOn { target: u, .. } => vec![u.clone()],
        Objective::Defend { unit, wave, .. } | Objective::Escort { unit, wave, .. } => {
            vec![unit.clone(), wave.clone()]
        }
        Objective::Loot(it, _) => content
            .units
            .iter()
            .filter(|u| u.loot.iter().any(|l| l.item == *it))
            .map(|u| u.key.clone())
            .collect(),
        Objective::ClearZone(z) => content
            .zone(z)
            .and_then(|z| z.boss.as_ref())
            .map(|b| vec![b.unit.clone()])
            .unwrap_or_default(),
        Objective::Wait { .. }
        | Objective::Interact(..)
        | Objective::Collect(..)
        | Objective::Inspect(..)
        | Objective::Sequence { .. } => Vec::new(),
    }
}

/// Unit keys the tracked quests still need: the minimap and map paint these in quest yellow.
pub fn objective_units(sim: &dyn SimRead, track: &QuestTrack) -> Vec<String> {
    let content = sim.content();
    let mut out: Vec<String> = Vec::new();
    for q in tracked(sim, track) {
        let Some(def) = content.quest(&q.id) else {
            continue;
        };
        for (k, o) in def.objectives.iter().enumerate() {
            if q.counts.get(k).copied().unwrap_or(0) >= o.need() {
                continue;
            }
            for key in objective_unit_keys(content, o) {
                if !out.contains(&key) {
                    out.push(key);
                }
            }
        }
    }
    out
}

/// One line of a unit's quest tooltip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TipQuest {
    /// "! Quest" to pick up or "? Quest" to hand in; `grey` for work still open elsewhere.
    Giver {
        glyph: &'static str,
        name: String,
        grey: bool,
    },
    /// A quest this unit counts toward, then its objective line; `done` once the count is full.
    Quest(String),
    Objective {
        text: String,
        done: bool,
    },
}

/// What a unit's hover tooltip says about the player's quests: givers first, then every open
/// quest the unit counts toward with the objective's progress (WoW's yellow lines).
pub fn unit_quest_lines(sim: &dyn SimRead, npc_key: &str, zone: &str) -> Vec<TipQuest> {
    let content = sim.content();
    let mut out = Vec::new();
    let chains = &content.calendar.chains;
    for (id, status) in giver_quests(sim, npc_key, zone) {
        let Some(def) = content.quest(&id) else {
            continue;
        };
        let (glyph, grey) = match status {
            QuestStatus::Available if chains.iter().any(|c| c.quests.first() == Some(&id)) => {
                continue;
            }
            QuestStatus::Available => ("!", false),
            QuestStatus::Ready => ("?", false),
            _ => ("?", true),
        };
        out.push(TipQuest::Giver {
            glyph,
            name: def.name.clone(),
            grey,
        });
    }
    for q in &sim.quests().active {
        let Some(def) = content.quest(&q.id) else {
            continue;
        };
        let mut head = false;
        for (k, o) in def.objectives.iter().enumerate() {
            if !objective_unit_keys(content, o).iter().any(|u| u == npc_key) {
                continue;
            }
            if !head {
                out.push(TipQuest::Quest(def.name.clone()));
                head = true;
            }
            let need = o.need();
            let have = q.counts.get(k).copied().unwrap_or(0).min(need);
            out.push(TipQuest::Objective {
                text: objective_line(content, o, have, need),
                done: have >= need,
            });
        }
    }
    out
}

/// What a quest prop's hover tooltip says: the open quests whose objectives use this prop
/// (by key or tag, like the waypoint arrow), and on the calendar board the chains to start.
pub fn prop_quest_lines(sim: &dyn SimRead, prop: &combat_core::world::PropDef) -> Vec<TipQuest> {
    let content = sim.content();
    let mut out = Vec::new();
    if prop.kind == combat_core::PropKind::Calendar {
        for c in &content.calendar.chains {
            let Some(def) = c.quests.first().and_then(|id| content.quest(id)) else {
                continue;
            };
            if sim.quest_status(&def.id) == QuestStatus::Available {
                out.push(TipQuest::Giver {
                    glyph: "!",
                    name: def.name.clone(),
                    grey: false,
                });
            }
        }
    }
    let tagged = !prop.tag.is_empty();
    for q in &sim.quests().active {
        let Some(def) = content.quest(&q.id) else {
            continue;
        };
        let mut head = false;
        for (k, o) in def.objectives.iter().enumerate() {
            let uses = match o {
                Objective::Interact(key, _) => *key == prop.key,
                Objective::Collect(tag, _) | Objective::Inspect(tag, _) => {
                    tagged && *tag == prop.tag
                }
                Objective::UseOn { target, .. } => {
                    *target == prop.key || (tagged && *target == prop.tag)
                }
                Objective::Sequence { props, .. } => props.contains(&prop.key),
                _ => false,
            };
            if !uses {
                continue;
            }
            if !head {
                out.push(TipQuest::Quest(def.name.clone()));
                head = true;
            }
            let need = o.need();
            let have = q.counts.get(k).copied().unwrap_or(0).min(need);
            out.push(TipQuest::Objective {
                text: objective_line(content, o, have, need),
                done: have >= need,
            });
        }
    }
    out
}

/// The open quest that wants bag or loot item `item` (looted, delivered or applied), if any.
pub fn quest_item_of(sim: &dyn SimRead, item: &str) -> Option<String> {
    let content = sim.content();
    sim.quests().active.iter().find_map(|q| {
        let def = content.quest(&q.id)?;
        def.objectives
            .iter()
            .any(|o| match o {
                Objective::Loot(it, _) => it == item,
                Objective::Deliver { item: it, .. } | Objective::UseOn { item: it, .. } => {
                    it == item
                }
                _ => false,
            })
            .then(|| def.name.clone())
    })
}

fn status_chip(s: QuestStatus) -> (&'static str, Color) {
    match s {
        QuestStatus::Available => ("TO DO", Color::srgb(0.7, 0.74, 0.82)),
        QuestStatus::Active => ("IN PROGRESS", Color::srgb(0.4, 0.65, 1.0)),
        // Violet: gold is the main-quest colour and a ready side quest must not borrow it.
        QuestStatus::Ready => ("IN REVIEW", Color::srgb(0.78, 0.6, 1.0)),
        QuestStatus::Done => ("DONE", DONE_GREEN),
        QuestStatus::Locked => ("BLOCKED", Color::srgb(1.0, 0.4, 0.35)),
    }
}

#[derive(Clone, PartialEq)]
enum Action {
    Select(String),
    Accept(String),
    Complete(String),
    Abandon(String),
    /// Gear reward pick in the open ticket.
    Choose(u8),
    ToggleTrack(String),
    /// Tracker title: open the log on that ticket.
    Open(String),
    /// Log row: point the quest arrow at this ticket.
    Guide(String),
    Collapse,
    Back,
    Close,
    CloseLog,
}

#[derive(Component)]
struct ActionButton(Action);
#[derive(Component)]
struct DialogWindow;
#[derive(Component)]
struct LogWindow;
#[derive(Component)]
struct Tracker;
#[derive(Component)]
struct TrackerArea;
#[derive(Component)]
struct ProgressToast;
#[derive(Component)]
struct TrophyHint;
#[derive(Component)]
struct TrophyRow;

#[derive(Resource, Default)]
struct DialogState {
    selected: Option<String>,
    cursor: usize,
    choice: u8,
    shown: String,
}

#[derive(Resource, Default)]
struct Shown {
    tracker: String,
    log: String,
    toast_age: f32,
}

pub struct QuestsPlugin;

impl Plugin for QuestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogState>()
            .init_resource::<Shown>()
            .init_resource::<QuestTrack>()
            .add_systems(Startup, spawn_quest_ui)
            .add_systems(
                Update,
                (
                    open_on_talk,
                    toggle_log,
                    handle_actions,
                    update_dialog,
                    update_log,
                    // Rebuilds strings and the tree from sim state; 10 Hz is plenty.
                    update_tracker.run_if(super::every(0.1)),
                    quest_events,
                    trophy_hint,
                )
                    .chain()
                    .in_set(SimReady),
            );
    }
}

/// Bottom margin that keeps the windows above the chat panel (hidden on touch).
fn chat_clear(touch: bool) -> f32 {
    if touch { 12.0 } else { 165.0 }
}

fn window_node(top: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(12),
        top: px(top),
        width: px(WINDOW_W),
        padding: UiRect::all(px(10)),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        border: UiRect {
            left: px(3),
            ..UiRect::all(px(1))
        },
        border_radius: BorderRadius::all(px(5)),
        overflow: Overflow::scroll_y(),
        display: Display::None,
        ..default()
    }
}

fn spawn_quest_ui(mut commands: Commands) {
    commands.spawn((
        DialogWindow,
        ScrollList::fit(12.0),
        window_node(132.0),
        BorderColor {
            left: GOLD,
            ..BorderColor::all(PANEL_EDGE)
        },
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(32),
    ));
    commands.spawn((
        LogWindow,
        ScrollList::fit(12.0),
        window_node(132.0),
        BorderColor {
            left: ACCENT,
            ..BorderColor::all(PANEL_EDGE)
        },
        panel_bg(),
        drop_shadow(),
        GlobalZIndex(31),
    ));
    // The area is laid out between the minimap and the bottom-right dock; its computed height
    // decides how many whole quests fit (`update_tracker`), whatever the screen size or UI scale.
    let area = commands
        .spawn((
            TrackerArea,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                // Below the minimap and the player's aura row (top right).
                top: px(TRACKER_TOP),
                bottom: px(DOCK_RESERVE),
                width: px(TRACKER_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        Tracker,
        crate::onboarding::Spot::Tracker,
        ChildOf(area),
        Node {
            padding: UiRect::all(px(8)),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(PANEL_EDGE.with_alpha(0.5)),
        BackgroundColor(Color::srgba(0.03, 0.035, 0.05, 0.8)),
        GlobalZIndex(20),
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: percent(23),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((ProgressToast, label("", 18.0), TextColor(Color::NONE)));
}

fn text(c: &mut ChildSpawnerCommands, s: impl Into<String>, size: f32, color: Color) {
    c.spawn((
        label(s, size),
        TextColor(color),
        Node {
            max_width: percent(100),
            ..default()
        },
    ));
}

fn section(c: &mut ChildSpawnerCommands, title: &str) {
    c.spawn((
        label(format!("// {title}"), 11.0),
        TextColor(ACCENT.with_alpha(0.85)),
        Node {
            margin: UiRect::top(px(4)),
            ..default()
        },
    ));
}

fn button(c: &mut ChildSpawnerCommands, key: &str, caption: &str, action: Action, primary: bool) {
    let color = if primary { GOLD } else { DIM };
    c.spawn((
        ActionButton(action),
        Button,
        Node {
            padding: UiRect::axes(px(10), px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(3)),
            column_gap: px(6),
            ..default()
        },
        BorderColor::all(color.with_alpha(0.8)),
        BackgroundColor(BAR_BG),
    ))
    .with_children(|b| {
        if !key.is_empty() {
            b.spawn((label(format!("[{key}]"), 12.0), TextColor(DIM)));
        }
        b.spawn((label(caption, 13.0), TextColor(color)));
    });
}

fn objectives(c: &mut ChildSpawnerCommands, content: &Content, sim: &dyn SimRead, id: &str) {
    let Some(def) = content.quest(id) else { return };
    let counts = sim
        .quests()
        .active
        .iter()
        .find(|q| q.id == id)
        .map(|q| q.counts.clone());
    let done = sim.quest_status(id) == QuestStatus::Done;
    for (k, o) in def.objectives.iter().enumerate() {
        let need = o.need();
        let have = if done {
            need
        } else {
            counts.as_ref().and_then(|c| c.get(k).copied()).unwrap_or(0)
        };
        let ok = have >= need;
        text(
            c,
            format!(
                "{} {}",
                if ok { "[x]" } else { "[ ]" },
                objective_line(content, o, have.min(need), need)
            ),
            13.0,
            if ok { DONE_GREEN } else { Color::WHITE },
        );
    }
    if def.group {
        text(
            c,
            "Group recommended: elite work. Bring a friend, or a lot of potions.",
            11.0,
            Color::srgb(1.0, 0.55, 0.25),
        );
    }
}

/// Reward block: XP and Kudos, the trophy (hover for its joke) and the "choose one" gear row.
fn reward(
    c: &mut ChildSpawnerCommands,
    sim: &dyn SimRead,
    id: &str,
    choice: u8,
    assets: &AssetServer,
    icons: &mut IconCache,
) {
    let content = sim.content();
    let Some(def) = content.quest(id) else { return };
    section(c, "reward");
    let sp = (u64::from(def.reward.xp) * u64::from(content.economy.quest_sp_pct) / 100) as u32
        + def.reward.sp;
    let xp = if def.reward.xp > 0 {
        format!("+{} XP   +{sp} Kudos", content.quest_xp(def))
    } else {
        "Exposure".into()
    };
    text(c, xp, 13.0, Color::srgb(0.75, 0.6, 1.0));
    if let Some((dept, n)) = &def.reward.rep {
        text(
            c,
            format!("+{n} {dept} standing"),
            12.0,
            Color::srgb(0.35, 0.7, 0.95),
        );
    }
    match def.reward.burnout {
        0 => {}
        b if b < 0 => text(c, format!("Burnout -{}%", -b / 10), 12.0, DONE_GREEN),
        b => text(
            c,
            format!("Burnout +{}% (paperwork is work)", b / 10),
            12.0,
            Color::srgb(1.0, 0.6, 0.35),
        ),
    }
    let role = sim
        .player()
        .and_then(|p| sim.unit(p))
        .and_then(|u| content.class_of_unit(&u.key))
        .map(|c| c.role);
    let choices = content.quest_gear_choices(def, role);
    if !choices.is_empty() {
        text(c, "Choose one:", 12.0, DIM);
        for (k, name) in choices.iter().enumerate() {
            let picked = usize::from(choice).min(choices.len() - 1) == k;
            let viewer = goods::Viewer {
                equipped: &[],
                role,
                level: 1,
                kudos: 0,
                tokens: 0,
                buying: false,
            };
            let lines = goods::tooltip(content, name, &viewer).main;
            c.spawn((
                ActionButton(Action::Choose(k as u8)),
                Button,
                Node {
                    column_gap: px(8),
                    padding: UiRect::all(px(4)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(if picked {
                    GOLD
                } else {
                    PANEL_EDGE.with_alpha(0.4)
                }),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|r| {
                r.spawn((
                    ImageNode::new(icons.get(assets, goods::icon_of(content, name))),
                    Node {
                        width: px(30),
                        height: px(30),
                        ..default()
                    },
                ));
                r.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|t| {
                    // Name, rarity/slot line and the stat lines; the rest lives in the bag tooltip.
                    for (k, (line, color)) in lines.iter().take(4).enumerate() {
                        t.spawn((
                            label(line.clone(), if k == 0 { 13.0 } else { 11.0 }),
                            TextColor(*color),
                        ));
                    }
                });
            });
        }
    }
    let Some(item) = def.reward.trophy.as_deref().and_then(|t| content.item(t)) else {
        return;
    };
    c.spawn((
        TrophyRow,
        Button,
        Node {
            column_gap: px(8),
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|r| {
        r.spawn((
            ImageNode::new(icons.get(assets, &item.icon)),
            Node {
                width: px(32),
                height: px(32),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BorderColor::all(GOLD),
        ));
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|t| {
            t.spawn((label(item.name.clone(), 13.0), TextColor(GOLD)));
            t.spawn((
                label(copy::bonus_text(&item.bonus.totals(1)), 12.0),
                TextColor(copy::BONUS_GREEN),
            ));
        });
    });
    c.spawn((
        TrophyHint,
        label(format!("\"{}\"", item.description), 12.0),
        TextColor(DIM),
        Node {
            display: Display::None,
            max_width: percent(100),
            ..default()
        },
    ));
}

fn open_on_talk(
    mut talked: MessageReader<Talked>,
    sim: SimView,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
) {
    for Talked(id) in talked.read() {
        let Some(u) = sim.unit(*id) else { continue };
        let list = giver_quests(&*sim, &u.key, &flow.zone);
        if list.is_empty() {
            continue;
        }
        windows.dialog = Some(*id);
        windows.log = false;
        state.selected = (list.len() == 1).then(|| list[0].0.clone());
        state.cursor = 0;
        state.choice = 0;
        state.shown.clear();
    }
}

fn toggle_log(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    mut windows: ResMut<QuestWindows>,
) {
    if !paused.0 && binds.just(&keys, crate::keybinds::Action::QuestLog) {
        windows.log = !windows.log;
        if windows.log {
            windows.dialog = None;
        }
    }
}

fn handle_actions(
    keys: Res<ButtonInput<KeyCode>>,
    paused: Res<Paused>,
    sim: SimView,
    flow: Res<GameFlow>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
    mut track: ResMut<QuestTrack>,
    mut pending: ResMut<PendingCommands>,
    buttons: Query<(&Interaction, &ActionButton), Changed<Interaction>>,
) {
    if paused.0 {
        return;
    }
    let mut actions: Vec<Action> = buttons
        .iter()
        .filter(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, b)| b.0.clone())
        .collect();
    // Keyboard drives the dialog only: Up/Down pick, Enter does the primary action, Backspace goes back.
    if let Some(npc) = windows.dialog.and_then(|id| sim.unit(id)) {
        let list = giver_quests(&*sim, &npc.key, &flow.zone);
        match &state.selected {
            None => {
                if keys.just_pressed(KeyCode::ArrowDown) {
                    state.cursor = (state.cursor + 1).min(list.len().saturating_sub(1));
                }
                if keys.just_pressed(KeyCode::ArrowUp) {
                    state.cursor = state.cursor.saturating_sub(1);
                }
                if keys.just_pressed(KeyCode::Enter)
                    && let Some((id, _)) = list.get(state.cursor)
                {
                    actions.push(Action::Select(id.clone()));
                }
            }
            Some(id) => {
                if keys.just_pressed(KeyCode::Enter) {
                    match sim.quest_status(id) {
                        QuestStatus::Available => actions.push(Action::Accept(id.clone())),
                        QuestStatus::Ready => actions.push(Action::Complete(id.clone())),
                        _ => actions.push(Action::Close),
                    }
                }
                if keys.just_pressed(KeyCode::Backspace) && list.len() > 1 {
                    actions.push(Action::Back);
                }
            }
        }
    }
    for action in actions {
        match action {
            Action::Select(id) => {
                state.selected = Some(id);
                state.choice = 0;
            }
            Action::Back => state.selected = None,
            Action::Close => windows.dialog = None,
            Action::CloseLog => {
                windows.log = false;
                track.focus = None;
            }
            Action::Abandon(quest) => pending.0.push(Command::AbandonQuest { quest }),
            Action::Choose(k) => state.choice = k,
            Action::ToggleTrack(id) => {
                if track.is_tracked(&id) {
                    track.untracked.push(id);
                } else {
                    track.untracked.retain(|u| *u != id);
                }
            }
            Action::Guide(id) => track.waypoint = Some(id),
            Action::Open(id) => {
                track.waypoint = Some(id.clone());
                track.focus = Some(id);
                windows.log = true;
                windows.dialog = None;
            }
            Action::Collapse => track.collapsed = !track.collapsed,
            Action::Accept(ref quest) | Action::Complete(ref quest) => {
                pending.0.push(if matches!(action, Action::Accept(_)) {
                    Command::AcceptQuest {
                        quest: quest.clone(),
                    }
                } else {
                    Command::TurnInQuest {
                        quest: quest.clone(),
                        choice: state.choice,
                    }
                });
                // Back to the list when the giver has more to hand out, else done talking.
                let more = windows
                    .dialog
                    .and_then(|id| sim.unit(id))
                    .map(|npc| giver_quests(&*sim, &npc.key, &flow.zone))
                    .is_some_and(|l| {
                        l.iter().any(|(q, s)| {
                            q != quest && matches!(s, QuestStatus::Available | QuestStatus::Ready)
                        })
                    });
                if more {
                    state.selected = None;
                    state.cursor = 0;
                } else {
                    windows.dialog = None;
                }
            }
        }
    }
}

fn update_dialog(
    mut commands: Commands,
    sim: SimView,
    flow: Res<GameFlow>,
    assets: Res<AssetServer>,
    mut icons: ResMut<IconCache>,
    mut windows: ResMut<QuestWindows>,
    mut state: ResMut<DialogState>,
    player: Single<&Transform, With<Player>>,
    touch: Res<TouchMode>,
    mut window: Single<(Entity, &mut Node, &mut ScrollList), With<DialogWindow>>,
) {
    let (root, node, scroll) = &mut *window;
    scroll.fit = Some(chat_clear(touch.active));
    let npc = windows.dialog.and_then(|id| sim.unit(id));
    let reach = sim.content().config.loot_range_m + combat_core::GIVER_REACH_SLACK_M;
    let near = npc.is_some_and(|u| {
        u.alive
            && Vec2::new(
                u.pos.x - player.translation.x,
                u.pos.z - player.translation.z,
            )
            .length()
                <= reach
    });
    let list = npc
        .map(|u| giver_quests(&*sim, &u.key, &flow.zone))
        .unwrap_or_default();
    if !near || list.is_empty() {
        windows.dialog = None;
        put!(node.display, Display::None);
        state.shown.clear();
        return;
    }
    let Some(npc) = npc else { return };
    // A quest that left the list (turned in) falls back to the list view.
    if let Some(sel) = &state.selected
        && !list.iter().any(|(q, _)| q == sel)
    {
        state.selected = None;
    }
    put!(node.display, Display::Flex);
    let content = sim.content();
    let counts: Vec<u32> = sim
        .quests()
        .active
        .iter()
        .flat_map(|q| q.counts.clone())
        .collect();
    let sig = format!(
        "{}|{:?}|{}|{}|{list:?}|{counts:?}",
        npc.id, state.selected, state.cursor, state.choice
    );
    // A new giver or quest page starts at the top; progress refreshes keep the offset.
    scroll.key = key_of(&format!("{}|{:?}", npc.id, state.selected));
    if sig == state.shown {
        return;
    }
    state.shown = sig;
    commands.entity(*root).despawn_related::<Children>();
    let npc_name = npc.name.clone();
    let selected = state.selected.clone();
    let cursor = state.cursor;
    let choice = state.choice;
    commands.entity(*root).with_children(|c| {
        let Some(id) = selected else {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|h| {
                h.spawn((
                    label(
                        format!("~/jira/board?reporter={}", asset_key(&npc_name)),
                        12.0,
                    ),
                    TextColor(DIM),
                ));
                button(h, "", "x", Action::Close, false);
            });
            text(
                c,
                format!("{npc_name} has tickets for you"),
                17.0,
                Color::WHITE,
            );
            for (k, (q, s)) in list.iter().enumerate() {
                let (chip, color) = status_chip(*s);
                let name = content.quest(q).map_or(q.as_str(), |d| d.name.as_str());
                let tint = if content.quest(q).is_some_and(|d| d.main) {
                    MAIN_MARK
                } else {
                    SIDE_MARK
                };
                let mark = match s {
                    QuestStatus::Ready => ("?", tint),
                    QuestStatus::Available => ("!", tint),
                    _ => ("?", GREY_MARK),
                };
                c.spawn((
                    ActionButton(Action::Select(q.clone())),
                    Button,
                    Node {
                        column_gap: px(8),
                        padding: UiRect::axes(px(6), px(4)),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(if k == cursor {
                        ACCENT
                    } else {
                        PANEL_EDGE.with_alpha(0.4)
                    }),
                    BackgroundColor(BAR_BG),
                ))
                .with_children(|r| {
                    r.spawn((label(mark.0, 16.0), TextColor(mark.1)));
                    r.spawn((label(ticket_id(content, q), 12.0), TextColor(DIM)));
                    r.spawn((label(name, 14.0), TextColor(Color::WHITE)));
                    r.spawn((label(chip, 10.0), TextColor(color)));
                });
            }
            text(c, "Up/Down pick | Enter open | Esc close", 11.0, DIM);
            return;
        };
        let Some(def) = content.quest(&id) else {
            return;
        };
        let status = sim.quest_status(&id);
        let (chip, chip_color) = status_chip(status);
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn(Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|l| {
                l.spawn((label(ticket_id(content, &id), 12.0), TextColor(ACCENT)));
                let (kind, kind_color) = kind_tag(def);
                l.spawn((label(kind, 11.0), TextColor(kind_color)));
                // Bracketed text, not a bordered box: the box mis-sized around 10 px text.
                l.spawn((label(format!("[{chip}]"), 10.0), TextColor(chip_color)));
            });
            button(h, "", "x", Action::Close, false);
        });
        text(c, def.name.clone(), 19.0, Color::WHITE);
        text(
            c,
            format!("Reporter: {npc_name}   Assignee: you   Priority: Highest (again)"),
            11.0,
            DIM,
        );
        section(c, "description");
        let body = match status {
            QuestStatus::Active => &def.progress_text,
            QuestStatus::Ready => &def.done_text,
            _ => &def.text,
        };
        text(c, body.clone(), 13.0, Color::srgb(0.86, 0.89, 0.95));
        section(c, "acceptance criteria");
        objectives(c, content, &*sim, &id);
        reward(c, &*sim, &id, choice, &assets, &mut icons);
        c.spawn(Node {
            column_gap: px(8),
            margin: UiRect::top(px(6)),
            ..default()
        })
        .with_children(|b| {
            match status {
                QuestStatus::Available => {
                    button(b, "Enter", "Accept", Action::Accept(id.clone()), true);
                }
                QuestStatus::Ready => {
                    button(b, "Enter", "Complete", Action::Complete(id.clone()), true);
                }
                _ => {}
            }
            if list.len() > 1 {
                button(b, "Bksp", "Back", Action::Back, false);
            }
            button(b, "Esc", "Close", Action::Close, false);
        });
    });
}

fn update_log(
    mut commands: Commands,
    sim: SimView,
    windows: Res<QuestWindows>,
    track: Res<QuestTrack>,
    mut shown: ResMut<Shown>,
    touch: Res<TouchMode>,
    mut window: Single<(Entity, &mut Node, &mut ScrollList), With<LogWindow>>,
) {
    let (root, node, scroll) = &mut *window;
    scroll.fit = Some(chat_clear(touch.active));
    if !windows.log {
        put!(node.display, Display::None);
        shown.log.clear();
        return;
    }
    put!(node.display, Display::Flex);
    let log = sim.quests();
    let sig = format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        log.active, log.completed, track.untracked, track.focus, track.waypoint
    );
    if sig == shown.log {
        return;
    }
    shown.log = sig;
    let content = sim.content();
    // The ticket clicked in the tracker comes first.
    // Main quests first, then side quests; stable sorts keep accept order inside a group.
    let mut order: Vec<&combat_core::ActiveQuest> = log.active.iter().collect();
    order.sort_by_key(|q| !content.quest(&q.id).is_some_and(|d| d.main));
    if let Some(f) = &track.focus {
        order.sort_by_key(|q| q.id != *f);
    }
    let mut last_main = None;
    commands.entity(*root).despawn_related::<Children>();
    commands.entity(*root).with_children(|c| {
        super::window_header(
            c,
            "Quests",
            super::subtitle("~/jira"),
            crate::keybinds::Action::QuestLog,
            ActionButton(Action::CloseLog),
        );
        let tracked_n = log
            .active
            .iter()
            .filter(|q| track.is_tracked(&q.id))
            .count();
        text(
            c,
            format!(
                "{} open ({tracked_n} tracked), {} closed. OKR status: green, decided in advance.",
                log.active.len(),
                log.completed.len()
            ),
            12.0,
            DIM,
        );
        if log.active.is_empty() {
            text(
                c,
                "Backlog empty. Look for a yellow ! before someone assigns you a chatbot.",
                13.0,
                Color::WHITE,
            );
        }
        for q in order {
            let Some(def) = content.quest(&q.id) else {
                continue;
            };
            if last_main != Some(def.main) {
                last_main = Some(def.main);
                section(
                    c,
                    if def.main {
                        "main quests"
                    } else {
                        "side quests"
                    },
                );
            }
            let status = sim.quest_status(&q.id);
            let (chip, color) = status_chip(status);
            let focused = track.focus.as_deref() == Some(q.id.as_str());
            let is_tracked = track.is_tracked(&q.id);
            c.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    padding: UiRect::all(px(6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BorderColor::all(if focused {
                    ACCENT
                } else {
                    PANEL_EDGE.with_alpha(0.6)
                }),
                BackgroundColor(BAR_BG),
            ))
            .with_children(|t| {
                t.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(6),
                    ..default()
                })
                .with_children(|h| {
                    h.spawn(Node {
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        flex_shrink: 1.0,
                        ..default()
                    })
                    .with_children(|l| {
                        l.spawn((label(ticket_id(content, &q.id), 11.0), TextColor(ACCENT)));
                        l.spawn((label(def.name.clone(), 14.0), TextColor(Color::WHITE)));
                        l.spawn((label(chip, 10.0), TextColor(color)));
                    });
                    h.spawn(Node {
                        column_gap: px(4),
                        ..default()
                    })
                    .with_children(|b| {
                        button(
                            b,
                            "",
                            if is_tracked { "Tracked" } else { "Track" },
                            Action::ToggleTrack(q.id.clone()),
                            is_tracked,
                        );
                        button(
                            b,
                            "",
                            "Guide",
                            Action::Guide(q.id.clone()),
                            track.waypoint.as_deref() == Some(q.id.as_str()),
                        );
                        button(b, "", "Abandon", Action::Abandon(q.id.clone()), false);
                    });
                });
                objectives(t, content, &*sim, &q.id);
                let hint = if status == QuestStatus::Ready {
                    format!("Turn in to {}.", unit_name(content, &def.ender().npc))
                } else {
                    def.progress_text.clone()
                };
                text(t, hint, 11.0, DIM);
            });
        }
    });
}

#[allow(clippy::too_many_arguments)] // Bevy system: one parameter per resource it reads
fn update_tracker(
    mut commands: Commands,
    time: Res<Time>,
    sim: SimView,
    windows: Res<QuestWindows>,
    trophies: Res<TrophiesOpen>,
    track: Res<QuestTrack>,
    touch: Res<TouchMode>,
    mut area: Single<(&mut Node, &ComputedNode), (With<TrackerArea>, Without<Tracker>)>,
    mut shown: ResMut<Shown>,
    mut tracker: Single<(Entity, &mut Node), With<Tracker>>,
    live: Res<crate::live::LiveTracker>,
    sprint: Res<crate::sprint::SprintHud>,
    minigame: Res<crate::minigame::MinigameHud>,
) {
    let (area_node, computed) = &mut *area;
    // Touch: the right side holds the buttons, so the tracker moves under the player frame
    // and stops above the stick.
    let (left, right, top, bottom) = if touch.active {
        (px(12), Val::Auto, px(TOUCH_TOP), px(TOUCH_RESERVE))
    } else {
        (Val::Auto, px(12), px(TRACKER_TOP), px(DOCK_RESERVE))
    };
    if area_node.left != left || area_node.top != top {
        area_node.left = left;
        area_node.right = right;
        area_node.top = top;
        area_node.bottom = bottom;
    }
    let room = computed.size().y * computed.inverse_scale_factor();
    let (root, node) = &mut *tracker;
    let list = tracked(&*sim, &track);
    // A sprint's run panel or a minigame header takes the tracker's slot, as WoW's challenge
    // timer does.
    let visible = (!list.is_empty() || !live.is_empty())
        && !trophies.0
        && !windows.map
        && !sprint.active
        && !minigame.active;
    put!(
        node.display,
        if visible {
            Display::Flex
        } else {
            Display::None
        }
    );
    let max = if touch.active {
        TRACKER_MAX_TOUCH
    } else {
        TRACKER_MAX
    };
    // Whole quests only, as many as fit the area; before its first layout it reads 0: show all.
    let room = if room > 0.0 { room } else { f32::MAX };
    // Whole seconds left on timed deliveries; part of the signature so the countdown ticks.
    let now = time.elapsed_secs();
    let clock: Vec<(&str, u32)> = track
        .deadlines
        .iter()
        .map(|(q, end)| (q.as_str(), (end - now).max(0.0).ceil() as u32))
        .collect();
    let sig = format!(
        "{list:?}|{}|{max}|{room:.0}|{}|{clock:?}",
        track.collapsed, live.sig
    );
    if !visible || sig == shown.tracker {
        return;
    }
    shown.tracker = sig;
    let content = sim.content();
    // Daily Standup lines and the Board Meeting countdown go first, then the quests that fit.
    let live_lines: usize =
        live.entries.iter().map(|e| e.2.len() + 1).sum::<usize>() + live.footer.len();
    let mut used = LINE_PX * (2 + live_lines) as f32;
    let entries: Vec<(String, String, bool, Vec<(String, Line)>)> = list
        .iter()
        .take(max)
        .filter_map(|q| {
            let def = content.quest(&q.id)?;
            let mut lines = tracker_lines(content, def, &q.counts);
            let left = clock.iter().find(|(id, _)| *id == q.id).map(|c| c.1);
            for (k, o) in def.objectives.iter().enumerate() {
                if let (Objective::Deliver { secs, .. }, Some(left), Some(line)) =
                    (o, left, lines.get_mut(k))
                    && *secs > 0
                    && line.1 == Line::Pending
                {
                    line.0 = format!("{} ({}:{:02})", line.0, left / 60, left % 60);
                }
            }
            Some((q.id.clone(), def.name.clone(), def.main, lines))
        })
        .take_while(|(_, _, _, lines)| {
            used += LINE_PX * (lines.len() + 1) as f32 + 3.0;
            used <= room
        })
        .collect();
    let more = list.len() - entries.len();
    let collapsed = track.collapsed;
    commands.entity(*root).despawn_related::<Children>();
    commands.entity(*root).with_children(|c| {
        c.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn((
                label(
                    // A footer-only tracker (a countdown) has no objectives to count.
                    match list.len() + live.entries.len() {
                        0 => "~/jira".to_string(),
                        n => format!("~/jira  Objectives ({n})"),
                    },
                    12.0,
                ),
                TextColor(ACCENT),
            ));
            h.spawn((
                ActionButton(Action::Collapse),
                Button,
                Node {
                    padding: UiRect::axes(px(6), px(0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                label(if collapsed { "[+]" } else { "[-]" }, 13.0),
                TextColor(DIM),
            ));
        });
        if collapsed {
            return;
        }
        for (title, color, lines) in &live.entries {
            c.spawn((
                label(title.clone(), 13.0),
                TextColor(*color),
                Node {
                    margin: UiRect::top(px(3)),
                    ..default()
                },
            ));
            for (line, state) in lines {
                let (mark, color) = match state {
                    Line::Pending => ("  - ", Color::srgb(0.82, 0.86, 0.92)),
                    Line::Done => ("  \u{2713} ", DONE_GREY),
                    Line::Ready => ("  ", GOLD),
                };
                text(c, format!("{mark}{line}"), 11.5, color);
            }
        }
        for (line, color) in &live.footer {
            text(c, format!("  {line}"), 11.0, *color);
        }
        for (id, name, main, lines) in entries {
            let ready = lines.last().is_some_and(|(_, l)| *l == Line::Ready);
            c.spawn((
                ActionButton(Action::Open(id)),
                Button,
                Node {
                    margin: UiRect::top(px(3)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                label(name, 13.0),
                TextColor(match (ready, main) {
                    (true, _) => GOLD,
                    (false, true) => MAIN_MARK,
                    (false, false) => Color::WHITE,
                }),
            ));
            for (line, state) in lines {
                let (mark, color) = match state {
                    Line::Pending => ("  - ", Color::srgb(0.82, 0.86, 0.92)),
                    Line::Done => ("  \u{2713} ", DONE_GREY),
                    Line::Ready => ("  ", GOLD),
                };
                text(c, format!("{mark}{line}"), 11.5, color);
            }
        }
        if more > 0 {
            text(c, format!("  +{more} more in ~/jira (J)"), 11.0, DIM);
        }
    });
}

fn quest_events(
    time: Res<Time>,
    sim: SimView,
    mut events: MessageReader<CombatEvent>,
    mut banners: MessageWriter<Banner>,
    mut shown: ResMut<Shown>,
    mut track: ResMut<QuestTrack>,
    mut toast: Single<(&mut Text, &mut TextColor), With<ProgressToast>>,
) {
    let content = sim.content();
    let (toast_text, toast_color) = &mut *toast;
    // A timed delivery seen without an accept (a loaded save) starts its clock now, as the
    // sim does.
    for q in &sim.quests().active {
        let secs = content.quest(&q.id).and_then(|d| {
            d.objectives.iter().find_map(|o| match o {
                Objective::Deliver { secs, .. } if *secs > 0 => Some(*secs),
                _ => None,
            })
        });
        if let Some(secs) = secs
            && !track.deadlines.iter().any(|(id, _)| *id == q.id)
        {
            let end = time.elapsed_secs() + secs as f32;
            track.deadlines.push((q.id.clone(), end));
        }
    }
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::QuestAccepted { quest } => {
                track.untracked.retain(|u| u != quest);
                track.deadlines.retain(|(q, _)| q != quest);
                let secs = content.quest(quest).and_then(|q| {
                    q.objectives.iter().find_map(|o| match o {
                        Objective::Deliver { secs, .. } if *secs > 0 => Some(*secs),
                        _ => None,
                    })
                });
                if let Some(secs) = secs {
                    let end = time.elapsed_secs() + secs as f32;
                    track.deadlines.push((quest.clone(), end));
                }
            }
            Event::QuestFailed { quest } => {
                track.deadlines.retain(|(q, _)| q != quest);
                if let Some(def) = content.quest(quest) {
                    banners.write(Banner {
                        title: format!("Ticket failed: {}", def.name),
                        subtitle: format!(
                            "Too slow. {} has another one if you want to try again.",
                            unit_name(content, &def.giver.npc)
                        ),
                        color: FAILED_RED,
                    });
                }
            }
            Event::QuestProgress {
                quest,
                objective,
                count,
                need,
            } => {
                if let Some(o) = content
                    .quest(quest)
                    .and_then(|q| q.objectives.get(*objective))
                    // The tracker bar shows a wait; a toast every second would only nag.
                    && !matches!(o, Objective::Wait { .. } | Objective::Defend { .. })
                {
                    set_text(toast_text, &progress_line(content, o, *count, *need));
                    shown.toast_age = 0.0;
                }
            }
            Event::QuestReady { quest } => {
                if let Some(def) = content.quest(quest) {
                    set_text(
                        toast_text,
                        &format!(
                            "{}: ready to close \u{2014} {} {}",
                            def.name,
                            if def.turn_in.is_some() {
                                "report to"
                            } else {
                                "return to"
                            },
                            unit_name(content, &def.ender().npc)
                        ),
                    );
                    shown.toast_age = 0.0;
                }
            }
            Event::QuestCompleted { quest } => {
                track.deadlines.retain(|(q, _)| q != quest);
                let Some(def) = content.quest(quest) else {
                    continue;
                };
                track.untracked.retain(|u| u != quest);
                banners.write(Banner {
                    title: format!("Ticket closed: {}", def.name),
                    subtitle: format!(
                        "+{} XP. Moved to Done. The AI will present it at the all-hands.",
                        content.quest_xp(def)
                    ),
                    color: DONE_GREEN,
                });
            }
            _ => {}
        }
    }
    shown.toast_age += time.delta_secs();
    let alpha = (1.0 - (shown.toast_age - TOAST_SECS * 0.6) / (TOAST_SECS * 0.4)).clamp(0.0, 1.0);
    put!(
        toast_color.0,
        GOLD.with_alpha(if toast_text.0.is_empty() { 0.0 } else { alpha })
    );
    if alpha == 0.0 && !toast_text.0.is_empty() {
        toast_text.0.clear();
    }
}

fn trophy_hint(
    rows: Query<&Interaction, With<TrophyRow>>,
    mut hints: Query<&mut Node, With<TrophyHint>>,
    mut buttons: Query<(&Interaction, &mut BackgroundColor), With<ActionButton>>,
) {
    let hovered = rows.iter().any(|i| *i != Interaction::None);
    for mut n in &mut hints {
        let d = if hovered {
            Display::Flex
        } else {
            Display::None
        };
        if n.display != d {
            n.display = d;
        }
    }
    for (i, mut bg) in &mut buttons {
        let c = match i {
            Interaction::None => BAR_BG,
            _ => Color::srgb(0.16, 0.18, 0.24),
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use combat_core::Sim;

    fn sim() -> Sim {
        match crate::combat::new_sim() {
            Ok(s) => s,
            Err(e) => panic!("{e:?}"),
        }
    }

    #[test]
    fn markers_follow_quest_status() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        // New hires get the intern's ticket in Car Park P3; he closes it on the campus.
        assert_eq!(
            giver_mark(&s, "intern", "parking_p3").map(|m| m.0),
            Some("!")
        );
        // The chain behind onboarding is locked, so Facilities has nothing yet.
        assert_eq!(giver_mark(&s, "facilities_guy", "campus"), None);
        s.debug_set_quest("onboarding", QuestStatus::Active);
        assert_eq!(giver_mark(&s, "intern", "campus"), Some(("?", GREY_MARK)));
        s.debug_set_quest("onboarding", QuestStatus::Ready);
        assert_eq!(giver_mark(&s, "intern", "campus"), Some(("?", MAIN_MARK)));
        assert!(giver_mark(&s, "intern", "lab/1").is_none());
        // Side quests get the other tint.
        s.debug_set_quest("onboarding", QuestStatus::Done);
        assert_eq!(giver_mark(&s, "barista", "campus"), Some(("!", SIDE_MARK)));
    }

    /// Whether the friendly `npc` stands in `zone` for a player at `level` whose log holds
    /// `done`/`open`. World progress the quests cannot imply (boss kills, zone clears) counts as
    /// met: the campaign got there before the quest did.
    fn stands_in(
        c: &Content,
        zone: &str,
        npc: &str,
        level: u8,
        done: &[String],
        open: &[String],
    ) -> bool {
        use combat_core::Unlock;
        fn met(u: &Unlock, level: u8, done: &[String], open: &[String]) -> bool {
            match u {
                Unlock::QuestDone(q) => done.contains(q),
                Unlock::QuestStarted(q) => done.contains(q) || open.contains(q),
                Unlock::PlayerLevel(n) => level >= *n,
                Unlock::AnyOf(any) => any.iter().any(|a| met(a, level, done, open)),
                Unlock::BossKilled(_) | Unlock::ZoneCleared(_) => true,
            }
        }
        c.zone(zone).is_some_and(|z| {
            z.friendly.iter().any(|f| {
                f.unit == npc
                    && f.requires
                        .as_ref()
                        .is_none_or(|r| met(r, level, done, open))
                    && !f.until.as_ref().is_some_and(|u| met(u, level, done, open))
            })
        })
    }

    /// Every quest the content can offer has a giver on the ground and a "!" over it the moment
    /// its gates open, and a "?" over whoever takes it once the work is done.
    #[test]
    fn every_quest_gets_its_marker_where_the_giver_stands() {
        let c = sim().content().clone();
        let mut bad = Vec::new();
        for q in &c.quests {
            let mut s = sim();
            s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
            s.set_player_level(q.level_min);
            // The chain behind it, oldest first.
            let mut done = Vec::new();
            let mut at = q.requires.clone();
            while let Some(id) = at {
                at = c.quest(&id).and_then(|d| d.requires.clone());
                done.push(id);
            }
            for id in &done {
                s.debug_set_quest(id, QuestStatus::Done);
            }
            if q.id == "il_00_wake" {
                s.debug_ilmari_opening();
            }
            // The ending's marker is written by the sim, never offered.
            if q.id == "il_w7_done" {
                continue;
            }
            if s.quest_status(&q.id) != QuestStatus::Available {
                bad.push(format!("{}: not available at level {}", q.id, q.level_min));
                continue;
            }
            let calendar_first = c
                .calendar
                .chains
                .iter()
                .any(|ch| ch.quests.first() == Some(&q.id));
            let g = &q.giver;
            if !stands_in(&c, &g.zone, &g.npc, q.level_min, &done, &[]) {
                bad.push(format!(
                    "{}: giver {} is not in {} when it opens",
                    q.id, g.npc, g.zone
                ));
            } else if !calendar_first && giver_mark(&s, &g.npc, &g.zone).map(|m| m.0) != Some("!") {
                bad.push(format!("{}: no ! over {}", q.id, g.npc));
            }
            s.debug_set_quest(&q.id, QuestStatus::Ready);
            let e = q.ender();
            let mut open = vec![q.id.clone()];
            open.extend(done.iter().cloned());
            if !stands_in(&c, &e.zone, &e.npc, q.level_min, &done, &open) {
                bad.push(format!(
                    "{}: taker {} is not in {} when it is done",
                    q.id, e.npc, e.zone
                ));
            } else if giver_mark(&s, &e.npc, &e.zone).map(|m| m.0) != Some("?") {
                bad.push(format!("{}: no ? over {}", q.id, e.npc));
            }
        }
        assert!(
            bad.is_empty(),
            "{} problems:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }

    #[test]
    fn a_giver_greys_out_a_few_levels_ahead_and_not_further() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        let c = s.content().clone();
        let q = c
            .quests
            .iter()
            .find(|q| q.requires.is_none() && q.level_min >= 3 && q.level_min <= 4)
            .expect("an early level-gated ticket");
        let (npc, zone) = (q.giver.npc.as_str(), q.giver.zone.as_str());
        s.set_player_level(q.level_min - 2);
        assert_eq!(giver_mark(&s, npc, zone), Some(("!", GREY_MARK)));
        s.set_player_level(q.level_min);
        assert_eq!(giver_mark(&s, npc, zone).map(|m| m.0), Some("!"));
        assert_ne!(giver_mark(&s, npc, zone).map(|m| m.1), Some(GREY_MARK));
        // Out of reach: no tease.
        let far = c
            .quests
            .iter()
            .find(|q| q.requires.is_none() && q.level_min > GREY_BANG_LEVELS + 1);
        if let Some(far) = far {
            s.set_player_level(1);
            let lone = c
                .quests
                .iter()
                .filter(|o| o.giver.npc == far.giver.npc && o.giver.zone == far.giver.zone)
                .all(|o| o.level_min > GREY_BANG_LEVELS + 1);
            if lone {
                assert_eq!(giver_mark(&s, &far.giver.npc, &far.giver.zone), None);
            }
        }
    }

    #[test]
    fn tracker_lines_read_like_wow() {
        let s = sim();
        let c = s.content();
        let badges = Objective::Loot("Access Badge".into(), 4);
        assert_eq!(progress_line(c, &badges, 3, 4), "Access Badges 3/4");
        let invoices = Objective::Loot("Token Invoice (Itemized)".into(), 4);
        assert_eq!(
            progress_line(c, &invoices, 1, 4),
            "Token Invoice (Itemized) 1/4"
        );
        let proxies = Objective::Kill("meatproxy".into(), 10);
        assert_eq!(
            objective_line(c, &proxies, 5, 10),
            "MeatProxies terminated 5/10"
        );
        let talk = Objective::Talk("facilities_guy".into());
        assert_eq!(objective_line(c, &talk, 0, 1), "Talk to Facilities Guy");
        assert_eq!(plural("Agile Coach"), "Agile Coaches");
        assert_eq!(plural("Copilot Drone"), "Copilot Drones");
        assert_eq!(
            plural("Gantt Chart (Pure Fiction)"),
            "Gantt Chart (Pure Fiction)"
        );
        assert_eq!(ticket_id(c, "onboarding"), "SPRINT-101");
        assert!(
            objective_label(c, &Objective::ClearZone("tower/1".into())).contains("Tower floor 1")
        );
        // Mid-progress: pending and done lines; all done adds the gold turn-in line.
        let Some(demo) = c.quest("demo_day") else {
            panic!("no demo_day")
        };
        let lines = tracker_lines(c, demo, &[3, 1]);
        assert_eq!(
            lines,
            [
                ("VC Partners terminated 3/3".to_string(), Line::Done),
                ("Demo Presenters terminated 1/3".to_string(), Line::Pending),
            ]
        );
        let lines = tracker_lines(c, demo, &[3, 3]);
        assert_eq!(
            lines.last(),
            Some(&(
                "Ready to close \u{2014} report to Peg Hollis".to_string(),
                Line::Ready
            ))
        );
        // A breadcrumb is closed at its target, not its giver.
        let Some(crumb) = c.quest("escalation_path") else {
            panic!("no escalation_path")
        };
        assert!(
            tracker_lines(c, crumb, &[1])
                .last()
                .is_some_and(|l| l.0.ends_with("IT Support Lead"))
        );
    }

    #[test]
    fn interactive_objectives_read_as_plain_tasks() {
        let s = sim();
        let c = s.content();
        let line = |q: &str, counts: &[u32]| {
            let Some(def) = c.quest(q) else {
                panic!("no {q}")
            };
            tracker_lines(c, def, counts)
                .first()
                .map(|l| l.0.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            line("passwords_on_post_its", &[2]),
            "Sticky Notes with a Password collected 2/4"
        );
        assert_eq!(
            line("fire_code_violations", &[1]),
            "Fire-Code Violations documented 1/4"
        );
        assert_eq!(
            line("have_you_tried_restarting", &[1]),
            "Restart the floor switches, A to C 1/3 \u{2014} next: Network Switch B (Humming)"
        );
        assert_eq!(
            line("meeting_room_reclamation", &[0]),
            "Use 'Room Released' Sticker on Meeting Room Panels 0/3"
        );
        assert_eq!(
            line("autocomplete_pest_control", &[1]),
            "Use Telemetry Opt-Out Form on Copilot Drones 1/3"
        );
        assert_eq!(
            line("coffee_for_the_noc", &[0]),
            "Deliver Lukewarm Coffee to NOC Operator"
        );
        assert!(
            line("performance_improvement_plan", &[40])
                .starts_with("Dave walked to his calibration [========")
        );
    }

    #[test]
    fn tracking_follows_accepts_and_untracks() {
        let mut s = sim();
        s.spawn_player("fullstack", combat_core::Vec3::new(0.0, 0.0, 0.0), 0.0);
        s.debug_set_quest("onboarding", QuestStatus::Active);
        s.debug_set_quest("coffee_run", QuestStatus::Active);
        let mut track = QuestTrack::default();
        assert_eq!(tracked(&s, &track).len(), 2);
        track.untracked.push("coffee_run".into());
        let ids: Vec<&str> = tracked(&s, &track).iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["onboarding"]);
        // Onboarding wants Facilities: his key is a highlighted objective unit.
        assert_eq!(objective_units(&s, &track), ["facilities_guy"]);
    }
}
