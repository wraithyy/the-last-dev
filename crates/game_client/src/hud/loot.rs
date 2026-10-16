//! Loot window and the quick "+item" feed.
//!
//! The sim hands a corpse's whole share to the looter in one `Loot` command and keeps on the corpse
//! only what did not fit the bag. So the window is a receipt plus a "Loot all" retry for leftovers,
//! never a pick-one list. With Auto Loot on it only opens for leftovers; the rest is a feed line.
//!
//! QA hooks: `WOW_LOOT_DEMO` shows the window for three sample items (display only, never auto-closes),
//! `WOW_AUTO_LOOT=0|1` overrides the setting.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use super::{
    ACCENT, DIM, IconCache, KEY_RIM, KEY_TOP, KEY_TOP_HI, drop_shadow, label, panel_bg, set_text,
    vgrad,
};
use crate::combat::{PendingCommands, SimReady, SimView};
use crate::copy;
use crate::economy::goods;
use crate::keybinds::{Action, Keybinds};
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use combat_core::{Command, EntityId};

/// A receipt with nothing left to take dismisses itself after this long.
const RECEIPT_SECS: f32 = 6.0;
/// The window outlives the loot range by this much so looting at the very edge does not flicker it.
const RANGE_SLACK: f32 = 0.5;
const TOAST_SECS: f32 = 2.6;
/// Close button box (HUD px): big enough to hit with a finger, not the 20x17 px text glyph it used to be.
const CLOSE_W: f32 = 34.0;
const CLOSE_H: f32 = 28.0;
const LOOT_ALL_H: f32 = 38.0;
const _: () = assert!(
    CLOSE_W >= 32.0 && CLOSE_H >= 28.0,
    "close button below a fingertip"
);

#[derive(Resource, Default)]
pub struct LootState {
    items: Vec<String>,
    sp: u32,
    pub dirty: bool,
    open: bool,
    corpse: Option<EntityId>,
    /// The corpse still holds items that did not fit.
    left: bool,
    /// Opened for leftovers: closes the moment the corpse is empty instead of after a timeout.
    had_left: bool,
    age: f32,
    /// Corpse of a Loot All in flight; its reply must not reopen the window.
    taking: Option<EntityId>,
    /// F was pressed while the window was open; handled by `update_loot`.
    take_request: bool,
    /// `WOW_LOOT_DEMO`: no corpse behind the window, so nothing auto-closes it.
    demo: bool,
}

/// How a `Looted` event is presented.
#[derive(PartialEq, Eq, Debug)]
pub enum Shown {
    Window,
    Feed,
    Nothing,
}

/// Everything that decides whether an open window goes away on its own.
struct Watch {
    /// Corpse still holds items.
    left: bool,
    had_left: bool,
    /// Distance to the corpse; `None` when it is gone.
    dist: Option<f32>,
    range: f32,
    engaged: bool,
    alive: bool,
    age: f32,
}

impl Watch {
    fn done(&self) -> bool {
        self.engaged
            || !self.alive
            || self.dist.is_none_or(|d| d > self.range + RANGE_SLACK)
            || (!self.left && (self.had_left || self.age >= RECEIPT_SECS))
    }
}

impl LootState {
    /// Decides how the player's `Looted` event shows. `left`: the corpse still holds items.
    pub fn on_looted(
        &mut self,
        corpse: EntityId,
        items: &[String],
        sp: u32,
        left: bool,
        auto: bool,
    ) -> Shown {
        let retry = self.taking == Some(corpse);
        if retry {
            self.taking = None;
        }
        if items.is_empty() && sp == 0 && !left {
            return Shown::Nothing;
        }
        if (auto || retry) && !left {
            return Shown::Feed;
        }
        self.items = items.to_vec();
        self.sp = sp;
        self.corpse = Some(corpse);
        self.left = left;
        self.had_left = left;
        self.age = 0.0;
        self.open = true;
        self.dirty = true;
        Shown::Window
    }

    pub fn dismiss(&mut self) {
        self.items.clear();
        self.sp = 0;
        self.corpse = None;
        self.left = false;
        self.had_left = false;
        self.open = false;
        self.demo = false;
        self.dirty = true;
    }

    /// Loot all: asks the sim for what stayed on the corpse (it applies range and ownership) and
    /// closes the window. Nothing is sent when the whole share was already taken.
    pub fn take_all(&mut self, me: EntityId) -> Option<Command> {
        let cmd = self.corpse.filter(|_| self.left).map(|corpse| {
            self.taking = Some(corpse);
            Command::Loot { looter: me, corpse }
        });
        self.dismiss();
        cmd
    }
}

impl LootState {
    /// A press on a window button; returns the command to send, if any.
    fn press(&mut self, btn: LootBtn, me: EntityId) -> Option<Command> {
        match btn {
            LootBtn::Close => {
                self.dismiss();
                None
            }
            LootBtn::All => self.take_all(me),
        }
    }
}

/// A toast line, "+ [Item]", shown when the window stays closed.
#[derive(Message)]
pub struct LootToast(pub String, pub Color);

#[derive(Component)]
struct LootWindow;
#[derive(Component)]
struct LootBody;
#[derive(Component)]
struct LootNote;
#[derive(Component)]
struct LootRow;
/// The item a loot row shows (hover or tap it for the comparison tooltip).
#[derive(Component)]
struct LootItem(String);
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum LootBtn {
    Close,
    All,
}
#[derive(Component)]
struct LootAllText;
#[derive(Component)]
struct ToastBox;
#[derive(Component)]
struct Toast(f32);

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<LootToast>()
            .init_resource::<LootState>()
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, keys_close.after(InputSystems))
            .add_systems(Update, dismiss_on_restart)
            .add_systems(
                Update,
                loot_demo
                    .in_set(SimReady)
                    .run_if(|| std::env::var_os("WOW_LOOT_DEMO").is_some()),
            )
            .add_systems(
                Update,
                (auto_close, update_loot, toasts).chain().in_set(SimReady),
            );
    }
}

fn spawn(mut commands: Commands) {
    // Fixed compact spot at the right edge: clear of the action bar (bottom centre), chat (bottom
    // left), the quest windows (left) and, via the body's `fit`, of the bottom-right dock.
    commands
        .spawn((
            LootWindow,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: percent(34),
                width: px(260),
                padding: UiRect::all(px(6)),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                display: Display::None,
                ..default()
            },
            // A press on the padding or the gaps between rows must not fall through to the world
            // (it used to select or deselect the unit behind the window).
            Interaction::default(),
            FocusPolicy::Block,
            BorderColor::all(ACCENT.with_alpha(0.7)),
            panel_bg(),
            drop_shadow(),
            // Above the quest, sprint and map windows (31..34) so none of them can cover the buttons.
            GlobalZIndex(39),
        ))
        .with_children(|c| {
            c.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|h| {
                h.spawn((label("// loot", 14.0), TextColor(DIM)));
                h.spawn((
                    LootBtn::Close,
                    Button,
                    Node {
                        width: px(CLOSE_W),
                        height: px(CLOSE_H),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BorderColor::all(KEY_RIM),
                    children![label("x", 15.0)],
                ));
            });
            // Only the rows scroll: the header and the Loot all button stay put, so a long corpse
            // can no longer scroll the close button out of the window.
            c.spawn((
                LootBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                super::scroll::ScrollList::fit(205.0),
            ));
            c.spawn((LootNote, label("", 11.0), TextColor(super::ACCENT_WARM)));
            c.spawn((
                LootBtn::All,
                Button,
                Node {
                    height: px(LOOT_ALL_H),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BorderColor::all(ACCENT),
                vgrad(KEY_TOP_HI, KEY_TOP),
                children![(LootAllText, label("Loot all  [F]", 16.0))],
            ));
        });
    // Above the action bar, centred; pure text, no hit-testing.
    commands.spawn((
        ToastBox,
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(22),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(2),
            ..default()
        },
        GlobalZIndex(12),
    ));
}

fn keys_close(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    binds: Res<Keybinds>,
    paused: Res<crate::input::Paused>,
    mut loot: ResMut<LootState>,
    mut inspect: ResMut<crate::economy::Inspect>,
) {
    // F again is Loot all. The press is eaten here, before `interact_input` sees it, otherwise
    // an empty receipt would turn it into "talk to the nearest colleague".
    let f = binds.key(Action::Interact);
    if loot.open && !paused.0 && keys.just_pressed(f) {
        keys.clear_just_pressed(f);
        loot.take_request = true;
    }
    if loot.open && keys.just_pressed(KeyCode::Escape) {
        // Eaten so the same press does not also clear the target or pause the game.
        keys.clear_just_pressed(KeyCode::Escape);
        loot.dismiss();
        inspect.loot = false;
        inspect.name = None;
    }
}

fn dismiss_on_restart(
    mut restarted: MessageReader<crate::game::GameRestarted>,
    mut loot: ResMut<LootState>,
) {
    if restarted.read().count() > 0 {
        loot.dismiss();
    }
}

fn auto_close(
    time: Res<Time>,
    sim: SimView,
    mut loot: ResMut<LootState>,
    mut inspect: ResMut<crate::economy::Inspect>,
) {
    if !loot.open || loot.demo {
        return;
    }
    loot.age += time.delta_secs();
    let Some(me) = sim.player().and_then(|p| sim.unit(p).map(|u| (p, u))) else {
        return;
    };
    let corpse = loot.corpse.and_then(|c| sim.unit(c));
    loot.left = corpse.is_some_and(|c| c.lootable);
    let watch = Watch {
        left: loot.left,
        had_left: loot.had_left,
        dist: corpse.map(|c| combat_core::distance(c.pos, me.1.pos)),
        range: sim.content().config.loot_range_m,
        engaged: sim
            .units()
            .any(|u| u.hostile && u.alive && u.in_combat && u.target == Some(me.0)),
        alive: me.1.alive,
        age: loot.age,
    };
    if watch.done() {
        loot.dismiss();
        inspect.loot = false;
        inspect.name = None;
    }
}

fn update_loot(
    (mut commands, assets, mut icons): (Commands, Res<AssetServer>, ResMut<IconCache>),
    sim: SimView,
    mut loot: ResMut<LootState>,
    mut window: Single<&mut Node, With<LootWindow>>,
    body: Single<Entity, With<LootBody>>,
    mut note: Single<&mut Text, With<LootNote>>,
    mut all_text: Single<&mut Text, (With<LootAllText>, Without<LootNote>)>,
    rows: Query<Entity, With<LootRow>>,
    presses: Query<
        (&Interaction, Option<&LootBtn>, Option<&LootItem>),
        (Or<(With<LootRow>, With<LootBtn>)>, Changed<Interaction>),
    >,
    hovered: Query<(&Interaction, &LootItem)>,
    (touch, touches): (
        Res<crate::touch::TouchMode>,
        Res<bevy::input::touch::Touches>,
    ),
    mut pending: ResMut<PendingCommands>,
    mut toasts: MessageWriter<LootToast>,
    mut hover: ResMut<crate::economy::HoverItem>,
    mut inspect: ResMut<crate::economy::Inspect>,
) {
    for (i, item) in &hovered {
        if *i != Interaction::None {
            hover.set(&item.0, false);
        }
    }
    let Some(me) = sim.player() else { return };
    // The touch HUD owns the right edge (minimap, Jump, action arcs); the window sits inboard.
    let (right, top) = if touch.active {
        (percent(26), percent(22))
    } else {
        (px(12), percent(34))
    };
    if window.right != right || window.top != top {
        window.right = right;
        window.top = top;
    }
    // Touch has no F key to advertise.
    set_text(
        &mut all_text,
        if touch.active {
            "Loot all"
        } else {
            "Loot all  [F]"
        },
    );
    let mut take = false;
    let mut close = false;
    for (i, btn, item) in &presses {
        if *i != Interaction::Pressed {
            continue;
        }
        match (btn, item) {
            (Some(LootBtn::Close), _) => close = true,
            (Some(LootBtn::All), _) => take = true,
            // The items are already in the bag; a row only inspects (first tap on touch).
            (None, Some(LootItem(name))) => {
                let act = inspect.gate(
                    touch.active,
                    name,
                    false,
                    crate::economy::tap_position(&touches),
                );
                inspect.loot = !act;
            }
            (None, None) => {}
        }
    }
    // F again takes what is left, as the key that opened the window (Space stays Jump).
    take |= std::mem::take(&mut loot.take_request);
    if loot.open && take {
        // The window vanishes, so say once more what went into the bag.
        for item in &loot.items {
            toasts.write(LootToast(
                format!("+ [{item}]"),
                goods::name_color(sim.content(), item),
            ));
        }
        if let Some(cmd) = loot.press(LootBtn::All, me) {
            pending.0.push(cmd);
        }
    } else if loot.open && close {
        loot.press(LootBtn::Close, me);
    }
    if !loot.dirty {
        return;
    }
    loot.dirty = false;
    for e in &rows {
        commands.entity(e).despawn();
    }
    if !loot.open {
        window.display = Display::None;
        inspect.loot = false;
        inspect.name = None;
        return;
    }
    let content = sim.content();
    let me_state = sim.unit(me);
    let equipped = me_state
        .map(|u| goods::slots_of(&u.equipped))
        .unwrap_or_default();
    let role = me_state
        .and_then(|u| content.class_of_unit(&u.key))
        .map(|c| c.role);
    set_text(
        &mut note,
        if loot.left {
            "Bag full: some items stayed on the corpse."
        } else {
            ""
        },
    );
    let mut lines: Vec<(String, Color, String, Color, &str)> = Vec::new();
    if loot.sp > 0 {
        lines.push((
            copy::kudos(loot.sp),
            goods::SP_GOLD,
            String::new(),
            DIM,
            "story_points",
        ));
    }
    for item in &loot.items {
        let color = goods::name_color(content, item);
        let (line, line_color) = match content.economy.goods(item) {
            Some(combat_core::Goods::Gear(g)) => {
                let mut l = format!("{} {}  ", g.rarity.label(), g.slot.label());
                l.push_str(&goods::stat_lines(&g.stats()).join(", "));
                let up = goods::is_upgrade(content, item, &equipped, role);
                if up {
                    l = format!("UPGRADE  {l}");
                }
                (
                    l,
                    if up {
                        goods::UPGRADE_GREEN
                    } else {
                        Color::WHITE
                    },
                )
            }
            Some(combat_core::Goods::Junk(j)) if j.ticket => (
                format!(
                    "Scrum board: {} Kudos",
                    j.price.saturating_mul(combat_core::TICKET_BOARD_MULT)
                ),
                goods::SP_GOLD,
            ),
            Some(g) => (
                format!(
                    "Sells for {} Kudos",
                    content.economy.sell_price(g.name()).unwrap_or(0)
                ),
                goods::SP_GOLD,
            ),
            None => (
                content
                    .item(item)
                    .map_or_else(String::new, |d| copy::bonus_text(&d.bonus.totals(1))),
                copy::BONUS_GREEN,
            ),
        };
        lines.push((
            item.clone(),
            color,
            line,
            line_color,
            goods::icon_of(content, item),
        ));
    }
    for (item, color, detail, detail_color, icon) in lines {
        let icon = icons.get(&assets, icon);
        let is_item = content.economy.goods(&item).is_some() || content.item(&item).is_some();
        let mut row = commands.spawn_empty();
        if is_item {
            row.insert(LootItem(item.clone()));
        }
        let row = row.id();
        commands.entity(*body).add_child(row);
        commands.entity(row).insert((
            LootRow,
            Button,
            Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            },
            children![
                (
                    ImageNode::new(icon),
                    Node {
                        width: px(30),
                        height: px(30),
                        flex_shrink: 0.0,
                        ..default()
                    }
                ),
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 1.0,
                        flex_basis: px(0),
                        flex_grow: 1.0,
                        ..default()
                    },
                    children![
                        (label(item, 13.0), TextColor(color)),
                        (label(detail, 11.0), TextColor(detail_color)),
                    ]
                ),
            ],
        ));
    }
    window.display = Display::Flex;
}

fn toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut new: MessageReader<LootToast>,
    boxed: Single<Entity, With<ToastBox>>,
    mut lines: Query<(Entity, &mut Toast, &mut TextColor)>,
) {
    for LootToast(text, color) in new.read() {
        let line = commands
            .spawn((
                Toast(0.0),
                label(text.clone(), 16.0),
                TextColor(*color),
                Node::default(),
            ))
            .id();
        commands.entity(*boxed).add_child(line);
    }
    for (e, mut t, mut color) in &mut lines {
        t.0 += time.delta_secs();
        if t.0 >= TOAST_SECS {
            commands.entity(e).despawn();
            continue;
        }
        // Hold, then fade out over the last 40 percent.
        let left = 1.0 - t.0 / TOAST_SECS;
        color.0 = color.0.with_alpha((left / 0.4).min(1.0));
    }
}

/// QA hook (`WOW_LOOT_DEMO`): shows the window and the matching log lines for three sample items
/// without a kill, so the layout can be screenshotted when no fight is available. Display only;
/// nothing is granted.
fn loot_demo(
    sim: SimView,
    mut loot: ResMut<LootState>,
    mut lines: MessageWriter<super::LogLineMsg>,
    mut done: Local<bool>,
) {
    if std::mem::replace(&mut *done, true) {
        return;
    }
    // A Jira ticket shows the junk path: no bonus, a hint where to close it.
    let ticket = "Jira Ticket: Won't Fix";
    lines.write(super::LogLineMsg(
        format!(
            "You receive: [{ticket}] Close it at the Scrum board or the Company Calendar for Kudos."
        ),
        goods::name_color(sim.content(), ticket),
    ));
    let mut items = vec![ticket.to_string()];
    for name in ["Prompt Fragment", "Expired GPU Credits"] {
        let Some(def) = sim.content().item(name) else {
            continue;
        };
        lines.write(super::LogLineMsg(
            format!(
                "You receive trophy: [{name}] ({})",
                copy::bonus_text(&def.bonus.totals(1))
            ),
            copy::BONUS_GREEN,
        ));
        items.push(name.to_string());
    }
    loot.items = items;
    loot.open = true;
    loot.demo = true;
    loot.dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<String> {
        vec!["Pasted Chat Transcript".into(), "Prompt Fragment".into()]
    }

    fn watch() -> Watch {
        Watch {
            left: false,
            had_left: false,
            dist: Some(1.0),
            range: 3.0,
            engaged: false,
            alive: true,
            age: 0.0,
        }
    }

    #[test]
    fn desktop_default_opens_the_window_with_the_receipt() {
        let mut s = LootState::default();
        assert_eq!(s.on_looted(7, &items(), 2, false, false), Shown::Window);
        assert!(s.open && s.dirty);
        assert_eq!(s.items.len(), 2);
        assert_eq!(s.sp, 2);
    }

    #[test]
    fn auto_loot_only_opens_the_window_for_what_did_not_fit() {
        let mut s = LootState::default();
        assert_eq!(s.on_looted(7, &items(), 2, false, true), Shown::Feed);
        assert!(!s.open);
        assert_eq!(s.on_looted(7, &items(), 0, true, true), Shown::Window);
        assert!(s.open && s.left && s.had_left);
        // Even nothing fitting at all is shown, otherwise the player never learns the bag is full.
        let mut s = LootState::default();
        assert_eq!(s.on_looted(8, &[], 0, true, true), Shown::Window);
        assert_eq!(s.on_looted(9, &[], 0, false, true), Shown::Nothing);
    }

    #[test]
    fn loot_all_takes_the_leftovers_and_closes() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 0, true, false);
        assert_eq!(
            s.take_all(1),
            Some(Command::Loot {
                looter: 1,
                corpse: 7
            })
        );
        assert!(!s.open && s.items.is_empty() && s.dirty);
        // The reply to our own Loot all must not pop the window up again.
        assert_eq!(s.on_looted(7, &items(), 0, false, false), Shown::Feed);
        assert!(!s.open);
    }

    #[test]
    fn loot_all_on_a_receipt_sends_nothing_and_closes() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 0, false, false);
        assert_eq!(s.take_all(1), None);
        assert!(!s.open);
    }

    #[test]
    fn a_second_corpse_swaps_the_window_content() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 0, false, false);
        s.on_looted(8, &["Expired GPU Credits".into()], 1, false, false);
        assert_eq!(s.corpse, Some(8));
        assert_eq!(s.items, vec!["Expired GPU Credits".to_string()]);
        assert_eq!(s.sp, 1);
    }

    #[test]
    fn closes_when_the_corpse_is_emptied() {
        assert!(!watch().done());
        let w = Watch {
            had_left: true,
            ..watch()
        };
        assert!(w.done());
        let w = Watch {
            had_left: true,
            left: true,
            ..watch()
        };
        assert!(!w.done());
    }

    #[test]
    fn closes_when_you_walk_out_of_range_or_the_corpse_is_gone() {
        let far = Watch {
            left: true,
            dist: Some(3.6),
            ..watch()
        };
        assert!(far.done());
        let edge = Watch {
            left: true,
            dist: Some(3.2),
            ..watch()
        };
        assert!(!edge.done());
        let gone = Watch {
            left: true,
            dist: None,
            ..watch()
        };
        assert!(gone.done());
    }

    #[test]
    fn closes_in_combat_when_dead_or_after_the_receipt_timeout() {
        let fight = Watch {
            left: true,
            engaged: true,
            ..watch()
        };
        assert!(fight.done());
        let dead = Watch {
            left: true,
            alive: false,
            ..watch()
        };
        assert!(dead.done());
        let late = Watch {
            age: RECEIPT_SECS,
            ..watch()
        };
        assert!(late.done());
        let held = Watch {
            left: true,
            age: RECEIPT_SECS * 5.0,
            ..watch()
        };
        assert!(!held.done());
    }

    #[test]
    fn dismiss_is_the_x_and_esc_path() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 3, true, false);
        s.dismiss();
        assert!(!s.open && s.items.is_empty() && s.sp == 0 && s.corpse.is_none());
        assert!(s.dirty);
    }

    #[test]
    fn x_press_closes_without_taking_anything() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 0, true, false);
        assert_eq!(s.press(LootBtn::Close, 1), None);
        assert!(!s.open && s.taking.is_none());
        // Pressing it with nothing open is harmless.
        assert_eq!(s.press(LootBtn::Close, 1), None);
    }

    #[test]
    fn loot_all_press_is_take_all() {
        let mut s = LootState::default();
        s.on_looted(7, &items(), 0, true, false);
        assert!(s.press(LootBtn::All, 1).is_some());
        assert!(!s.open);
    }
}
