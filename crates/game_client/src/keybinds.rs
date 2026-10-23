//! Rebindable keys. Gameplay systems ask `Keybinds` for the physical `KeyCode` of an `Action`
//! instead of hard-coding one, so Czech and other layouts keep working (positions, not letters).
//! To add a bindable action: add a variant, then fill `all`, `id`, `label`, `group` and
//! `default_key`; the panel, persistence and keycap labels pick it up from there.
//! The `~/keybindings` panel (Esc menu) lives in `panel`.

mod panel;

use bevy::prelude::*;
use std::collections::HashMap;

pub use panel::KeybindsPlugin;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Forward,
    Backward,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Jump,
    /// Rush to Standup (`world.spells`).
    Sprint,
    /// Mount or leave the e-scooter.
    Scooter,
    TargetNext,
    /// 0 targets yourself, 1..=4 the other group members in join order.
    Party(u8),
    /// Number-row action bar slot, 0..10.
    Slot(u8),
    /// Numpad alternative of the same slot.
    NumSlot(u8),
    AutoAttack,
    Coffee,
    Resurrect,
    /// The class's off-bar utility (the Data Scientist's `Ground Truth`).
    Utility,
    Interact,
    QuestLog,
    Map,
    Trophies,
    /// `~/whoami`: equipment, stats and bag.
    CharacterSheet,
    /// `~/career`: the Career Ladder talent window.
    Career,
    /// `~/pets`: the pet journal.
    Pets,
    /// `~/endgame`: Promotion rank, reputations, lockouts and the weekly seed.
    Endgame,
    /// `~/calendar`: the Company Calendar (minigames and quest chains).
    Calendar,
    /// Gadget bar slot 0..3: equipped on-use items.
    Gadget(u8),
    Chat,
    CombatLog,
    Meter,
    Emotes,
    Mute,
    MinimapZoomIn,
    MinimapZoomOut,
    /// `~/perf $ top`: cycles off, compact, full.
    Perf,
    /// Accepts the "drop to a lower preset" offer the perf watchdog shows.
    QualityDown,
}

/// Panel column a group is drawn in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Movement,
    Targeting,
    Combat,
    Windows,
    Bar,
    Numpad,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Movement => "movement",
            Group::Targeting => "targeting",
            Group::Combat => "combat",
            Group::Windows => "windows and misc",
            Group::Bar => "action bar",
            Group::Numpad => "numpad bar",
        }
    }
}

/// The 0-based slot digit as the keycap shows it: slots run 1..9 then 0.
fn digit(i: u8) -> u8 {
    (i + 1) % 10
}

impl Action {
    /// Panel order.
    pub fn all() -> Vec<Action> {
        use Action::*;
        let mut v = vec![
            Forward,
            Backward,
            TurnLeft,
            TurnRight,
            StrafeLeft,
            StrafeRight,
            Jump,
            Sprint,
            Scooter,
            TargetNext,
        ];
        v.extend((0..5).map(Party));
        v.extend([AutoAttack, Coffee, Resurrect, Utility, Interact]);
        v.extend([
            QuestLog,
            Map,
            Trophies,
            CharacterSheet,
            Career,
            Pets,
            Endgame,
            Calendar,
            Chat,
            CombatLog,
            Meter,
            Emotes,
            Mute,
            MinimapZoomIn,
            MinimapZoomOut,
            Perf,
            QualityDown,
        ]);
        v.extend((0..3).map(Gadget));
        v.extend((0..10).map(Slot));
        v.extend((0..10).map(NumSlot));
        v
    }

    /// Stable key in the saved file; never rename.
    pub fn id(self) -> String {
        use Action::*;
        match self {
            Forward => "forward".into(),
            Backward => "backward".into(),
            TurnLeft => "turn_left".into(),
            TurnRight => "turn_right".into(),
            StrafeLeft => "strafe_left".into(),
            StrafeRight => "strafe_right".into(),
            Jump => "jump".into(),
            Sprint => "sprint".into(),
            Scooter => "scooter".into(),
            TargetNext => "target_next".into(),
            Party(i) => format!("party{i}"),
            Slot(i) => format!("slot{i}"),
            NumSlot(i) => format!("numslot{i}"),
            AutoAttack => "auto_attack".into(),
            Coffee => "coffee".into(),
            Resurrect => "resurrect".into(),
            Utility => "utility".into(),
            Interact => "interact".into(),
            QuestLog => "quest_log".into(),
            Map => "map".into(),
            Trophies => "trophies".into(),
            CharacterSheet => "character_sheet".into(),
            Career => "career".into(),
            Pets => "pets".into(),
            Endgame => "endgame".into(),
            Calendar => "calendar".into(),
            Gadget(i) => format!("gadget{i}"),
            Chat => "chat".into(),
            CombatLog => "combat_log".into(),
            Meter => "meter".into(),
            Emotes => "emotes".into(),
            Mute => "mute".into(),
            MinimapZoomIn => "minimap_zoom_in".into(),
            MinimapZoomOut => "minimap_zoom_out".into(),
            Perf => "perf".into(),
            QualityDown => "quality_down".into(),
        }
    }

    pub fn label(self) -> String {
        use Action::*;
        match self {
            Forward => "move forward".into(),
            Backward => "move back".into(),
            TurnLeft => "turn / strafe left".into(),
            TurnRight => "turn / strafe right".into(),
            StrafeLeft => "strafe left".into(),
            StrafeRight => "strafe right".into(),
            Jump => "jump".into(),
            Sprint => "rush to standup (sprint)".into(),
            Scooter => "e-scooter (mount)".into(),
            TargetNext => "target next".into(),
            Party(0) => "target self".into(),
            Party(i) => format!("target party {i}"),
            Slot(i) => format!("slot {}", digit(i)),
            NumSlot(i) => format!("numpad slot {}", digit(i)),
            AutoAttack => "auto attack".into(),
            Coffee => "coffee break".into(),
            Resurrect => "resurrect (reboot)".into(),
            Utility => "class utility (Ground Truth)".into(),
            Interact => "talk / loot".into(),
            QuestLog => "~/jira quest log".into(),
            Map => "map".into(),
            Trophies => "trophies".into(),
            CharacterSheet => "~/whoami gear and bag".into(),
            Career => "~/career talents".into(),
            Pets => "~/pets journal".into(),
            Endgame => "~/endgame ranks and reputation".into(),
            Calendar => "~/calendar side quests and minigames".into(),
            Gadget(i) => format!("gadget {} (on-use gear)", i + 1),
            Chat => "chat window".into(),
            CombatLog => "combat log".into(),
            Meter => "meter window".into(),
            Emotes => "emote menu".into(),
            Mute => "mute".into(),
            MinimapZoomIn => "minimap zoom in".into(),
            MinimapZoomOut => "minimap zoom out".into(),
            Perf => "~/perf window".into(),
            QualityDown => "accept lower quality offer".into(),
        }
    }

    pub fn group(self) -> Group {
        use Action::*;
        match self {
            Forward | Backward | TurnLeft | TurnRight | StrafeLeft | StrafeRight | Jump
            | Sprint | Scooter => Group::Movement,
            TargetNext | Party(_) => Group::Targeting,
            AutoAttack | Coffee | Resurrect | Utility | Interact | Gadget(_) => Group::Combat,
            Slot(_) => Group::Bar,
            NumSlot(_) => Group::Numpad,
            QuestLog | Map | Trophies | CharacterSheet | Career | Pets | Endgame | Calendar
            | Chat | CombatLog | Meter | Emotes | Mute | MinimapZoomIn | MinimapZoomOut | Perf
            | QualityDown => Group::Windows,
        }
    }

    pub fn default_key(self) -> KeyCode {
        use Action::*;
        match self {
            Forward => KeyCode::KeyW,
            Backward => KeyCode::KeyS,
            TurnLeft => KeyCode::KeyA,
            TurnRight => KeyCode::KeyD,
            StrafeLeft => KeyCode::KeyQ,
            StrafeRight => KeyCode::KeyE,
            Jump => KeyCode::Space,
            Sprint => KeyCode::KeyX,
            Scooter => KeyCode::KeyY,
            TargetNext => KeyCode::Tab,
            Party(i) => [
                KeyCode::F1,
                KeyCode::F2,
                KeyCode::F3,
                KeyCode::F4,
                KeyCode::F5,
            ][usize::from(i).min(4)],
            Slot(i) => DIGITS[usize::from(i).min(9)],
            NumSlot(i) => NUMPAD[usize::from(i).min(9)],
            AutoAttack => KeyCode::KeyT,
            Coffee => KeyCode::KeyC,
            Resurrect => KeyCode::KeyV,
            // Every letter is taken; the comma sits beside the calendar's period.
            Utility => KeyCode::Comma,
            Interact => KeyCode::KeyF,
            QuestLog => KeyCode::KeyJ,
            Map => KeyCode::KeyN,
            Trophies => KeyCode::KeyB,
            CharacterSheet => KeyCode::KeyI,
            // R for Review: the performance review that decides promotions.
            Career => KeyCode::KeyR,
            // Z for zoo: P is already the third gadget.
            Pets => KeyCode::KeyZ,
            // The tilde key: `~/endgame`.
            Endgame => KeyCode::Backquote,
            // Every letter is taken; the period sits where a calendar dot would.
            Calendar => KeyCode::Period,
            Gadget(i) => [KeyCode::KeyU, KeyCode::KeyO, KeyCode::KeyP][usize::from(i).min(2)],
            Chat => KeyCode::KeyH,
            CombatLog => KeyCode::KeyL,
            Meter => KeyCode::KeyK,
            Emotes => KeyCode::KeyG,
            Mute => KeyCode::KeyM,
            MinimapZoomIn => KeyCode::Equal,
            MinimapZoomOut => KeyCode::Minus,
            // Function keys are physical positions, so every layout agrees on them.
            Perf => KeyCode::F6,
            QualityDown => KeyCode::F7,
        }
    }
}

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];
const NUMPAD: [KeyCode; 10] = [
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
    KeyCode::Numpad0,
];

/// (key, name in the saved file, short keycap label). Only keys listed here can be bound:
/// Esc, Enter and Backspace belong to menus and F9 to the dev overlay.
macro_rules! key_table {
    ($($code:ident => $label:expr),* $(,)?) => {
        const KEYS: &[(KeyCode, &str, &str)] =
            &[$((KeyCode::$code, stringify!($code), $label)),*];
    };
}

key_table! {
    KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D", KeyE => "E", KeyF => "F", KeyG => "G",
    KeyH => "H", KeyI => "I", KeyJ => "J", KeyK => "K", KeyL => "L", KeyM => "M", KeyN => "N",
    KeyO => "O", KeyP => "P", KeyQ => "Q", KeyR => "R", KeyS => "S", KeyT => "T", KeyU => "U",
    KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y", KeyZ => "Z",
    Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4",
    Digit5 => "5", Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
    Numpad0 => "N0", Numpad1 => "N1", Numpad2 => "N2", Numpad3 => "N3", Numpad4 => "N4",
    Numpad5 => "N5", Numpad6 => "N6", Numpad7 => "N7", Numpad8 => "N8", Numpad9 => "N9",
    NumpadAdd => "N+", NumpadSubtract => "N-", NumpadMultiply => "N*", NumpadDivide => "N/",
    NumpadDecimal => "N.",
    F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6", F7 => "F7",
    F8 => "F8", F10 => "F10", F11 => "F11", F12 => "F12",
    Space => "Spc", Tab => "Tab", Backquote => "~", Minus => "-", Equal => "=",
    BracketLeft => "[", BracketRight => "]", Semicolon => ";", Quote => "'", Comma => ",",
    Period => ".", Slash => "/", Backslash => "\\", IntlBackslash => "<>",
    ArrowUp => "Up", ArrowDown => "Down", ArrowLeft => "Left", ArrowRight => "Right",
    Home => "Home", End => "End", PageUp => "PgUp", PageDown => "PgDn", Insert => "Ins",
    Delete => "Del", ShiftLeft => "LShift", ShiftRight => "RShift",
}

fn key_name(k: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|e| e.0 == k).map(|e| e.1)
}

fn key_from_name(name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|e| e.1 == name).map(|e| e.0)
}

/// Short text for a keycap or hint; unbindable keys fall back to the code name.
pub fn key_label(k: KeyCode) -> String {
    KEYS.iter()
        .find(|e| e.0 == k)
        .map_or_else(|| format!("{k:?}"), |e| e.2.to_owned())
}

/// Outcome of a rebind attempt, for the panel's message line.
#[derive(Debug, PartialEq, Eq)]
pub enum Rebind {
    Done,
    /// The key was in use; that action now has the old key of the rebound one.
    Swapped(Action),
    Rejected(&'static str),
}

#[derive(Resource, Clone, PartialEq, Debug)]
pub struct Keybinds {
    keys: HashMap<Action, KeyCode>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            keys: Action::all()
                .into_iter()
                .map(|a| (a, a.default_key()))
                .collect(),
        }
    }
}

impl Keybinds {
    pub fn key(&self, a: Action) -> KeyCode {
        self.keys
            .get(&a)
            .copied()
            .unwrap_or_else(|| a.default_key())
    }

    pub fn label(&self, a: Action) -> String {
        key_label(self.key(a))
    }

    pub fn just(&self, keys: &ButtonInput<KeyCode>, a: Action) -> bool {
        keys.just_pressed(self.key(a))
    }

    pub fn held(&self, keys: &ButtonInput<KeyCode>, a: Action) -> bool {
        keys.pressed(self.key(a))
    }

    /// Number row and numpad key of action bar slot `i`.
    pub fn slot_keys(&self, i: usize) -> [KeyCode; 2] {
        let i = u8::try_from(i).unwrap_or(u8::MAX);
        [self.key(Action::Slot(i)), self.key(Action::NumSlot(i))]
    }

    pub fn owner_of(&self, k: KeyCode) -> Option<Action> {
        Action::all().into_iter().find(|a| self.key(*a) == k)
    }

    /// Binds `k` to `a`. A key already used by another action is swapped, so every key keeps
    /// exactly one meaning and nothing is ever left unbound.
    pub fn rebind(&mut self, a: Action, k: KeyCode) -> Rebind {
        if key_name(k).is_none() {
            return Rebind::Rejected("that key is reserved");
        }
        if self.key(a) == k {
            return Rebind::Done;
        }
        let old = self.key(a);
        let other = self.owner_of(k);
        self.keys.insert(a, k);
        match other {
            Some(o) => {
                self.keys.insert(o, old);
                Rebind::Swapped(o)
            }
            None => Rebind::Done,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn serialize(&self) -> String {
        Action::all()
            .into_iter()
            .filter_map(|a| Some(format!("{}={}\n", a.id(), key_name(self.key(a))?)))
            .collect()
    }

    /// Tolerant of unknown or stale lines; a file that makes two actions share a key is
    /// discarded whole (defaults), since a half-trusted map could lock a control out.
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        let by_id: HashMap<String, Action> =
            Action::all().into_iter().map(|a| (a.id(), a)).collect();
        let mut from_file = std::collections::HashSet::new();
        for line in text.lines() {
            let Some((id, name)) = line.split_once('=') else {
                continue;
            };
            if let (Some(a), Some(k)) = (by_id.get(id.trim()), key_from_name(name.trim())) {
                out.keys.insert(*a, k);
                from_file.insert(*a);
            }
        }
        // An action added after the file was written keeps its default unless the player
        // already uses that key; then it moves to a free one instead of voiding the file.
        for a in Action::all() {
            let k = out.key(a);
            let taken = out.keys.iter().any(|(o, ok)| *o != a && *ok == k);
            if !from_file.contains(&a) && taken {
                let used: Vec<KeyCode> = out.keys.values().copied().collect();
                if let Some(free) = KEYS.iter().map(|e| e.0).find(|c| !used.contains(c)) {
                    out.keys.insert(a, free);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        if out.keys.values().all(|k| seen.insert(*k)) {
            out
        } else {
            Self::default()
        }
    }
}

/// Text on a keycap or a hint that follows the bindings.
#[derive(Component)]
pub struct KeyLegend(pub Action);

fn sync_legends(binds: Res<Keybinds>, mut legends: Query<(&KeyLegend, &mut Text)>) {
    for (l, mut text) in &mut legends {
        crate::hud::set_text(&mut text, &binds.label(l.0));
    }
}

/// While the panel waits for a key, the pressed key belongs to it alone.
#[derive(Resource, Default)]
pub struct KeyCapture(pub Option<Action>);

fn install(app: &mut App) {
    let loaded = crate::net::load_keys().map_or_else(Keybinds::default, |t| Keybinds::parse(&t));
    app.insert_resource(loaded)
        .init_resource::<KeyCapture>()
        .add_systems(Update, sync_legends);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_action_never_voids_an_old_file_that_uses_its_key() {
        let b = Keybinds::parse("jump=Backquote\nforward=KeyW\n");
        assert_eq!(b.key(Action::Jump), KeyCode::Backquote);
        assert_ne!(b.key(Action::Endgame), KeyCode::Backquote);
        assert_eq!(Keybinds::default().key(Action::Endgame), KeyCode::Backquote);
    }

    #[test]
    fn defaults_are_unique_and_bindable() {
        let b = Keybinds::default();
        let mut seen = std::collections::HashSet::new();
        for a in Action::all() {
            assert!(key_name(b.key(a)).is_some(), "{a:?} default not in table");
            assert!(seen.insert(b.key(a)), "{a:?} shares its default key");
        }
    }

    #[test]
    fn rebind_to_free_key() {
        let mut b = Keybinds::default();
        assert_eq!(
            b.rebind(Action::AutoAttack, KeyCode::NumpadDecimal),
            Rebind::Done
        );
        assert_eq!(b.key(Action::AutoAttack), KeyCode::NumpadDecimal);
        assert_eq!(b.owner_of(KeyCode::KeyT), None);
    }

    #[test]
    fn conflict_swaps_keys() {
        let mut b = Keybinds::default();
        assert_eq!(
            b.rebind(Action::AutoAttack, KeyCode::KeyC),
            Rebind::Swapped(Action::Coffee)
        );
        assert_eq!(b.key(Action::AutoAttack), KeyCode::KeyC);
        assert_eq!(b.key(Action::Coffee), KeyCode::KeyT);
    }

    #[test]
    fn reserved_keys_are_rejected() {
        let mut b = Keybinds::default();
        assert!(matches!(
            b.rebind(Action::Map, KeyCode::Escape),
            Rebind::Rejected(_)
        ));
        assert_eq!(b.key(Action::Map), KeyCode::KeyN);
    }

    #[test]
    fn round_trips_and_resets() {
        let mut b = Keybinds::default();
        b.rebind(Action::Map, KeyCode::NumpadDecimal);
        b.rebind(Action::Slot(0), KeyCode::KeyY);
        assert_eq!(Keybinds::parse(&b.serialize()), b);
        b.reset();
        assert_eq!(b, Keybinds::default());
    }

    #[test]
    fn parse_tolerates_garbage_and_rejects_duplicates() {
        let b = Keybinds::parse("nonsense\nmap=NumpadDecimal\nunknown=KeyQ\njump=NoSuchKey\n");
        assert_eq!(b.key(Action::Map), KeyCode::NumpadDecimal);
        assert_eq!(b.key(Action::Jump), KeyCode::Space);
        // map and quest_log on one key: the whole file is distrusted.
        assert_eq!(
            Keybinds::parse("map=NumpadDecimal\nquest_log=NumpadDecimal\n"),
            Keybinds::default()
        );
    }

    #[test]
    fn slot_keys_follow_bindings() {
        let mut b = Keybinds::default();
        assert_eq!(b.slot_keys(0), [KeyCode::Digit1, KeyCode::Numpad1]);
        b.rebind(Action::Slot(0), KeyCode::KeyX);
        assert_eq!(b.slot_keys(0)[0], KeyCode::KeyX);
        assert_eq!(b.slot_keys(9)[0], KeyCode::Digit0);
    }
}
