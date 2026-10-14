//! `~/metrics (Q3 velocity report)`: a damage meter fed by the same `CombatEvent`s as the combat log,
//! so it works offline (just you) and online (every group member: the server sends damage and heal
//! events to all clients; only quest, loot, XP and own cast failures are private).
//!
//! A fight runs from the first counted event to `END_GRACE` seconds after combat ends; the last
//! fight stays visible until the next one starts. All aggregation is in `Meter` (pure, tested).

use super::logs::{PARTY_BLUE, Win, close_button};
use super::{BAR_BG, DIM, KEY_RIM, KEY_TOP, PANEL_EDGE, drop_shadow, label, set_text};
use crate::combat::{CombatEvent, SimReady, SimView};
use bevy::prelude::*;
use combat_core::{EntityId, Event, MAX_PLAYERS};
use std::collections::BTreeMap;

/// Seconds without combat before a fight is closed; its length still ends at the last activity.
const END_GRACE: f64 = 4.0;
/// DPS over fights shorter than this would explode on the first hit.
const MIN_SECS: f64 = 1.0;
const WIDTH: f32 = 270.0;
const BAR_H: f32 = 17.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Damage,
    Healing,
    Taken,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Self::Damage => Self::Healing,
            Self::Healing => Self::Taken,
            Self::Taken => Self::Damage,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Damage => "Story points burned",
            Self::Healing => "Incidents resolved",
            Self::Taken => "Blame absorbed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scope {
    #[default]
    Current,
    Overall,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stat {
    Damage,
    Healing,
    Taken,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Totals {
    pub damage: u64,
    pub healing: u64,
    pub taken: u64,
}

impl Totals {
    fn get(&self, mode: Mode) -> u64 {
        match mode {
            Mode::Damage => self.damage,
            Mode::Healing => self.healing,
            Mode::Taken => self.taken,
        }
    }

    fn add(&mut self, o: &Totals) {
        self.damage += o.damage;
        self.healing += o.healing;
        self.taken += o.taken;
    }
}

type Table = BTreeMap<EntityId, Totals>;

fn merge(into: &mut Table, from: &Table) {
    for (id, t) in from {
        into.entry(*id).or_default().add(t);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fight {
    pub table: Table,
    pub secs: f64,
}

#[derive(Clone, Debug)]
struct Active {
    start: f64,
    last_active: f64,
    table: Table,
}

#[derive(Default, Debug)]
pub struct Meter {
    active: Option<Active>,
    last: Fight,
    overall: Fight,
}

impl Meter {
    /// Counts `amount` for `who`; the first counted event of a lull opens a fight.
    pub fn record(&mut self, now: f64, who: EntityId, stat: Stat, amount: u32) {
        let a = self.active.get_or_insert_with(|| Active {
            start: now,
            last_active: now,
            table: Table::new(),
        });
        a.last_active = now;
        let t = a.table.entry(who).or_default();
        let n = u64::from(amount);
        match stat {
            Stat::Damage => t.damage += n,
            Stat::Healing => t.healing += n,
            Stat::Taken => t.taken += n,
        }
    }

    /// Call every frame: `in_combat` keeps the fight alive, silence for `END_GRACE` closes it.
    pub fn tick(&mut self, now: f64, in_combat: bool) {
        let Some(a) = &mut self.active else { return };
        if in_combat {
            a.last_active = now;
        } else if now - a.last_active >= END_GRACE {
            let done = Fight {
                table: std::mem::take(&mut a.table),
                secs: (a.last_active - a.start).max(0.0),
            };
            self.active = None;
            merge(&mut self.overall.table, &done.table);
            self.overall.secs += done.secs;
            self.last = done;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn in_fight(&self) -> bool {
        self.active.is_some()
    }

    /// Current: the running fight, else the last finished one. Overall: every fight since the reset.
    pub fn view(&self, scope: Scope) -> Fight {
        let running = self.active.as_ref().map(|a| Fight {
            table: a.table.clone(),
            secs: (a.last_active - a.start).max(0.0),
        });
        match (scope, running) {
            (Scope::Current, Some(r)) => r,
            (Scope::Current, None) => self.last.clone(),
            (Scope::Overall, running) => {
                let mut all = self.overall.clone();
                if let Some(r) = running {
                    merge(&mut all.table, &r.table);
                    all.secs += r.secs;
                }
                all
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Entry {
    pub id: EntityId,
    pub value: u64,
    pub per_sec: f64,
}

/// One entry per listed player (zero rows included), best first; ties keep id order.
pub fn rank(fight: &Fight, mode: Mode, players: &[EntityId]) -> Vec<Entry> {
    let secs = fight.secs.max(MIN_SECS);
    let mut out: Vec<Entry> = players
        .iter()
        .map(|id| {
            let value = fight.table.get(id).map_or(0, |t| t.get(mode));
            Entry {
                id: *id,
                value,
                per_sec: value as f64 / secs,
            }
        })
        .collect();
    out.sort_by(|a, b| b.value.cmp(&a.value).then(a.id.cmp(&b.id)));
    out
}

/// "950", "1.2k", "3.4M".
pub fn short_num(n: u64) -> String {
    match n {
        0..1000 => n.to_string(),
        1000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[derive(Resource, Default)]
struct MeterState {
    meter: Meter,
    mode: Mode,
    scope: Scope,
}

#[derive(Component)]
struct ModeButton;
#[derive(Component)]
struct ModeText;
#[derive(Component)]
struct ScopeButton;
#[derive(Component)]
struct ScopeText;
#[derive(Component)]
struct ResetButton;
#[derive(Component)]
struct BarRoot(usize);
#[derive(Component)]
struct BarFill(usize);
#[derive(Component)]
struct BarName(usize);
#[derive(Component)]
struct BarValue(usize);
#[derive(Component)]
struct FightClock;

pub struct MeterPlugin;

impl Plugin for MeterPlugin {
    fn build(&self, app: &mut App) {
        // QA hook: `WOW_METER_MODE=0|1|2` (damage, healing, taken) and `WOW_METER_SCOPE=overall`.
        let mode = match std::env::var("WOW_METER_MODE").as_deref() {
            Ok("1") => Mode::Healing,
            Ok("2") => Mode::Taken,
            _ => Mode::Damage,
        };
        let scope = if std::env::var_os("WOW_METER_SCOPE").is_some() {
            Scope::Overall
        } else {
            Scope::Current
        };
        app.insert_resource(MeterState {
            mode,
            scope,
            ..default()
        })
        .add_systems(Startup, spawn_meter)
        .add_systems(
            Update,
            (feed_meter, buttons, update_meter).chain().in_set(SimReady),
        );
    }
}

fn small_button() -> impl Bundle {
    (
        Button,
        Node {
            height: px(16),
            padding: UiRect::horizontal(px(6)),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        BackgroundColor(KEY_TOP),
    )
}

fn button_label(text: &str) -> impl Bundle {
    (label(text, 10.0), TextColor(Color::srgb(0.9, 0.95, 1.0)))
}

fn spawn_meter(mut commands: Commands) {
    commands
        .spawn((
            Win::Meter,
            Node {
                position_type: PositionType::Absolute,
                width: px(WIDTH),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            drop_shadow(),
        ))
        .with_children(|root| {
            // No window dots here: the long satirical title needs the width.
            root.spawn((
                Node {
                    height: px(20),
                    padding: UiRect::horizontal(px(8)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.15, 0.2, 0.96)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                    label("~/metrics (Q3 velocity report)", 10.0),
                    TextColor(DIM),
                ));
                close_button(bar, Win::Meter);
            });
            root.spawn((
                Node {
                    padding: UiRect::axes(px(6), px(5)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.88)),
            ))
            .with_children(|body| {
                body.spawn(Node {
                    column_gap: px(4),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((ModeButton, small_button())).with_children(|b| {
                        b.spawn((ModeText, button_label("")));
                    });
                    row.spawn((ScopeButton, small_button())).with_children(|b| {
                        b.spawn((ScopeText, button_label("")));
                    });
                    row.spawn((ResetButton, small_button())).with_children(|b| {
                        b.spawn(button_label("reset"));
                    });
                });
                body.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|bars| {
                    for i in 0..MAX_PLAYERS {
                        bars.spawn((
                            BarRoot(i),
                            Node {
                                height: px(BAR_H),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::horizontal(px(4)),
                                overflow: Overflow::clip(),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(BAR_BG),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                BarFill(i),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0),
                                    top: px(0),
                                    bottom: px(0),
                                    width: percent(0),
                                    ..default()
                                },
                                BackgroundColor(PARTY_BLUE.with_alpha(0.55)),
                            ));
                            bar.spawn((BarName(i), label("", 10.0)));
                            bar.spawn((BarValue(i), label("", 10.0)));
                        });
                    }
                });
                body.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|foot| {
                    foot.spawn((label("Numbers may be AI-generated", 9.0), TextColor(DIM)));
                    foot.spawn((FightClock, label("", 9.0), TextColor(DIM)));
                });
            });
        });
}

fn feed_meter(
    time: Res<Time>,
    mut events: MessageReader<CombatEvent>,
    mut restarted: MessageReader<crate::game::GameRestarted>,
    sim: SimView,
    mut state: ResMut<MeterState>,
    mut owner: Local<Option<EntityId>>,
) {
    let now = time.elapsed_secs_f64();
    // Entity ids differ per session: a new game or a mode switch starts a clean report.
    let me = sim.player();
    if restarted.read().count() > 0 || *owner != me {
        *owner = me;
        state.meter.reset();
        events.clear();
    }
    let players = sim.players();
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::Damage {
                source,
                target,
                amount,
                ..
            } => {
                if players.contains(source) {
                    state.meter.record(now, *source, Stat::Damage, *amount);
                }
                if players.contains(target) {
                    state.meter.record(now, *target, Stat::Taken, *amount);
                }
            }
            Event::Heal { source, amount, .. } if players.contains(source) => {
                state.meter.record(now, *source, Stat::Healing, *amount);
            }
            _ => {}
        }
    }
    let in_combat = players
        .iter()
        .any(|id| sim.unit(*id).is_some_and(|u| u.in_combat));
    state.meter.tick(now, in_combat);
}

fn buttons(
    mut state: ResMut<MeterState>,
    mode: Query<&Interaction, (With<ModeButton>, Changed<Interaction>)>,
    scope: Query<&Interaction, (With<ScopeButton>, Changed<Interaction>)>,
    reset: Query<&Interaction, (With<ResetButton>, Changed<Interaction>)>,
) {
    if mode.iter().any(|i| *i == Interaction::Pressed) {
        state.mode = state.mode.next();
    }
    if scope.iter().any(|i| *i == Interaction::Pressed) {
        state.scope = match state.scope {
            Scope::Current => Scope::Overall,
            Scope::Overall => Scope::Current,
        };
    }
    if reset.iter().any(|i| *i == Interaction::Pressed) {
        state.meter.reset();
    }
}

fn update_meter(
    sim: SimView,
    state: Res<MeterState>,
    mut mode_text: Single<&mut Text, (With<ModeText>, Without<ScopeText>)>,
    mut scope_text: Single<&mut Text, (With<ScopeText>, Without<ModeText>)>,
    mut clock_text: Single<&mut Text, (With<FightClock>, Without<ModeText>, Without<ScopeText>)>,
    mut roots: Query<(&BarRoot, &mut Node), Without<BarFill>>,
    mut fills: Query<(&BarFill, &mut Node, &mut BackgroundColor), Without<BarRoot>>,
    mut names: Query<
        (&BarName, &mut Text),
        (
            Without<BarValue>,
            Without<ModeText>,
            Without<ScopeText>,
            Without<FightClock>,
        ),
    >,
    mut values: Query<
        (&BarValue, &mut Text),
        (
            Without<BarName>,
            Without<ModeText>,
            Without<ScopeText>,
            Without<FightClock>,
        ),
    >,
) {
    let fight = state.meter.view(state.scope);
    let ranked = rank(&fight, state.mode, sim.players());
    let max = ranked.first().map_or(0, |e| e.value).max(1);
    let me = sim.player();
    set_text(&mut mode_text, state.mode.title());
    set_text(
        &mut scope_text,
        match state.scope {
            Scope::Current => "current",
            Scope::Overall => "overall",
        },
    );
    let live = if state.meter.in_fight() {
        " (live)"
    } else {
        ""
    };
    set_text(&mut clock_text, &format!("{}{live}", clock(fight.secs)));
    for (BarRoot(i), mut node) in &mut roots {
        let want = if *i < ranked.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (BarFill(i), mut node, mut bg) in &mut fills {
        // Role colour from the class system; party blue until a unit has one.
        let tint = ranked
            .get(*i)
            .and_then(|e| sim.unit(e.id))
            .and_then(|u| crate::classes::role_of_unit(sim.content(), &u.key))
            .map_or(PARTY_BLUE, crate::classes::role_color)
            .with_alpha(0.55);
        if bg.0 != tint {
            bg.0 = tint;
        }
        let pct = ranked
            .get(*i)
            .map_or(0.0, |e| 100.0 * e.value as f32 / max as f32);
        let want = percent(pct);
        if node.width != want {
            node.width = want;
        }
    }
    for (BarName(i), mut text) in &mut names {
        let line = ranked.get(*i).map_or_else(String::new, |e| {
            let name = sim
                .unit(e.id)
                .map_or_else(|| "someone".to_string(), |u| u.name.clone());
            let you = if Some(e.id) == me { " (you)" } else { "" };
            format!("{}. {name}{you}", i + 1)
        });
        set_text(&mut text, &line);
    }
    for (BarValue(i), mut text) in &mut values {
        let line = ranked.get(*i).map_or_else(String::new, |e| {
            format!("{}  {:.1}/s", short_num(e.value), e.per_sec)
        });
        set_text(&mut text, &line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: EntityId = 1;
    const B: EntityId = 2;

    #[test]
    fn a_fight_runs_from_first_event_to_its_last_activity() {
        let mut m = Meter::default();
        m.record(10.0, A, Stat::Damage, 50);
        m.tick(10.0, true);
        m.record(14.0, A, Stat::Damage, 70);
        m.tick(14.0, true);
        assert!(m.in_fight());
        // Out of combat but within the grace: still the same fight.
        m.tick(16.0, false);
        assert!(m.in_fight());
        m.tick(18.5, false);
        assert!(!m.in_fight());
        let f = m.view(Scope::Current);
        assert_eq!(
            f.secs, 4.0,
            "length stops at the last activity, not the grace"
        );
        assert_eq!(f.table[&A].damage, 120);
        // The finished fight stays visible until a new one starts.
        let again = m.view(Scope::Current);
        assert_eq!(again, f);
    }

    #[test]
    fn sums_are_per_player_and_per_stat() {
        let mut m = Meter::default();
        m.record(0.0, A, Stat::Damage, 10);
        m.record(0.0, B, Stat::Healing, 7);
        m.record(1.0, B, Stat::Taken, 5);
        m.record(1.0, A, Stat::Damage, 5);
        let f = m.view(Scope::Current);
        assert_eq!(
            f.table[&A],
            Totals {
                damage: 15,
                healing: 0,
                taken: 0
            }
        );
        assert_eq!(
            f.table[&B],
            Totals {
                damage: 0,
                healing: 7,
                taken: 5
            }
        );
    }

    #[test]
    fn current_switches_to_the_new_fight_and_overall_accumulates() {
        let mut m = Meter::default();
        m.record(0.0, A, Stat::Damage, 100);
        m.tick(2.0, true);
        m.tick(7.0, false);
        m.record(20.0, A, Stat::Damage, 30);
        m.tick(22.0, true);
        assert_eq!(m.view(Scope::Current).table[&A].damage, 30);
        let all = m.view(Scope::Overall);
        assert_eq!(all.table[&A].damage, 130);
        assert_eq!(all.secs, 2.0 + 2.0);
    }

    #[test]
    fn rates_divide_by_fight_length_with_a_floor_and_rank_orders_players() {
        let mut m = Meter::default();
        m.record(0.0, A, Stat::Damage, 100);
        m.record(10.0, B, Stat::Damage, 300);
        let f = m.view(Scope::Current);
        let r = rank(&f, Mode::Damage, &[A, B, 3]);
        assert_eq!(r.iter().map(|e| e.id).collect::<Vec<_>>(), [B, A, 3]);
        assert_eq!(r[0].per_sec, 30.0);
        assert_eq!(r[2].value, 0, "idle group members still get a row");
        // A single instant hit must not divide by zero.
        let mut quick = Meter::default();
        quick.record(5.0, A, Stat::Damage, 40);
        let q = rank(&quick.view(Scope::Current), Mode::Damage, &[A]);
        assert_eq!(q[0].per_sec, 40.0);
        // Another mode reads another column.
        assert_eq!(rank(&f, Mode::Healing, &[A])[0].value, 0);
    }

    #[test]
    fn reset_clears_current_last_and_overall() {
        let mut m = Meter::default();
        m.record(0.0, A, Stat::Damage, 100);
        m.tick(9.0, false);
        m.record(30.0, A, Stat::Damage, 1);
        m.reset();
        assert!(!m.in_fight());
        assert!(m.view(Scope::Current).table.is_empty());
        assert!(m.view(Scope::Overall).table.is_empty());
        assert_eq!(m.view(Scope::Overall).secs, 0.0);
    }

    #[test]
    fn numbers_and_clock_are_short() {
        assert_eq!(short_num(950), "950");
        assert_eq!(short_num(1234), "1.2k");
        assert_eq!(short_num(3_400_000), "3.4M");
        assert_eq!(clock(75.9), "1:15");
        assert_eq!(Mode::Taken.next(), Mode::Damage);
    }
}
