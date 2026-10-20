//! The "First Day" step machine and its copy, free of Bevy so it can be tested on its own.
//! The glue in `mod.rs` turns real input and sim events into `Signal`s; a step only advances on
//! the signal it asks for, never on a timer.

use combat_core::Role;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Move,
    Camera,
    Accept,
    Target,
    AutoAttack,
    Spell1,
    Spell2,
    Interrupt,
    RoleTip,
    Loot,
    TurnIn,
    Windows,
    Scooter,
    Handoff,
}

pub const STEPS: [Step; 14] = [
    Step::Move,
    Step::Camera,
    Step::Accept,
    Step::Target,
    Step::AutoAttack,
    Step::Spell1,
    Step::Spell2,
    Step::Interrupt,
    Step::RoleTip,
    Step::Loot,
    Step::TurnIn,
    Step::Windows,
    Step::Scooter,
    Step::Handoff,
];

/// Walking this far from where the step began counts as having learnt to walk.
pub const MOVE_M: f32 = 4.0;
/// A ranged hit from at least this far counts as kiting.
pub const KITE_M: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Window {
    Map,
    Sheet,
    Career,
}

impl Window {
    fn bit(self) -> u8 {
        match self {
            Window::Map => 1,
            Window::Sheet => 2,
            Window::Career => 4,
        }
    }
}

/// Something the player actually did, as observed by the glue.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Signal {
    /// Metres walked since the step began.
    Moved(f32),
    CameraMoved,
    QuestAccepted,
    TargetedDummy,
    AutoAttackOn,
    /// A spell cast from this bar slot landed.
    CastSlot(u8),
    /// The dummy's training video stopped casting, either way.
    VideoEnded {
        interrupted: bool,
    },
    /// Took a hit with part of it absorbed.
    Soaked,
    HealedColleague,
    /// A Compliance Zone landed and missed the player.
    DodgedZone,
    /// A spell hit the dummy from this far away (m).
    RangedHit(f32),
    LootedDummy,
    QuestTurnedIn,
    Opened(Window),
    Mounted,
    TalkedToIntern,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tutorial {
    pub step: usize,
    pub role: Role,
    /// The player knows an interrupt: the interrupt step then wants a real interrupt.
    pub can_interrupt: bool,
    windows: u8,
}

impl Tutorial {
    pub fn new(role: Role, can_interrupt: bool) -> Self {
        Tutorial {
            step: 0,
            role,
            can_interrupt,
            windows: 0,
        }
    }

    pub fn current(&self) -> Option<Step> {
        STEPS.get(self.step).copied()
    }

    /// Marks a window as seen (touch has no gear button, so that one is not asked for).
    pub fn skip_window(&mut self, w: Window) {
        self.windows |= w.bit();
    }

    pub fn done(&self) -> bool {
        self.step >= STEPS.len()
    }

    /// Windows of the `Windows` step still to open.
    pub fn windows_left(&self) -> Vec<Window> {
        [Window::Map, Window::Sheet, Window::Career]
            .into_iter()
            .filter(|w| self.windows & w.bit() == 0)
            .collect()
    }

    /// True when the signal finished the current step.
    pub fn feed(&mut self, sig: Signal) -> bool {
        let Some(step) = self.current() else {
            return false;
        };
        let ok = match (step, sig) {
            (Step::Move, Signal::Moved(m)) => m >= MOVE_M,
            (Step::Camera, Signal::CameraMoved) => true,
            (Step::Accept, Signal::QuestAccepted) => true,
            (Step::Target, Signal::TargetedDummy) => true,
            (Step::AutoAttack, Signal::AutoAttackOn) => true,
            (Step::Spell1, Signal::CastSlot(0)) => true,
            (Step::Spell2, Signal::CastSlot(1)) => true,
            (Step::Interrupt, Signal::VideoEnded { interrupted }) => {
                interrupted || !self.can_interrupt
            }
            (Step::RoleTip, s) => {
                matches!(
                    (self.role, s),
                    (Role::Tank, Signal::Soaked)
                        | (Role::Healer, Signal::HealedColleague)
                        | (Role::Melee, Signal::DodgedZone)
                ) || matches!((self.role, s), (Role::Ranged, Signal::RangedHit(d)) if d >= KITE_M)
            }
            (Step::Loot, Signal::LootedDummy) => true,
            (Step::TurnIn, Signal::QuestTurnedIn) => true,
            (Step::Windows, Signal::Opened(w)) => {
                self.windows |= w.bit();
                self.windows_left().is_empty()
            }
            (Step::Scooter, Signal::Mounted) => true,
            (Step::Handoff, Signal::TalkedToIntern) => true,
            _ => false,
        };
        if ok {
            self.step += 1;
        }
        ok
    }
}

/// Labels and names the copy needs, resolved by the glue from keybinds and content.
pub struct Ctx {
    pub touch: bool,
    pub move_keys: String,
    pub target: String,
    pub auto: String,
    pub interact: String,
    pub map: String,
    pub sheet: String,
    pub career: String,
    pub scooter: String,
    /// (spell name, key) of bar slots 1 and 2.
    pub spell1: (String, String),
    pub spell2: (String, String),
    /// The class interrupt: (name, key, learn level).
    pub interrupt: Option<(String, String, u8)>,
    /// The role step's spell: absorb for tanks, heal for healers, slot 1 otherwise.
    pub role_spell: (String, String),
    /// The tank's taunt and the level it unlocks at.
    pub taunt: Option<(String, u8)>,
    /// One extra sentence for the role tip of classes whose kit works differently.
    pub class_tip: Option<String>,
}

/// One step's panel copy.
pub struct Copy {
    pub title: &'static str,
    pub how: String,
    pub quip: &'static str,
}

pub fn copy(t: &Tutorial, c: &Ctx) -> Option<Copy> {
    let step = t.current()?;
    let pick = |desk: String, touch: &str| if c.touch { touch.to_string() } else { desk };
    let (title, how, quip) = match step {
        Step::Move => (
            "walk to your desk (you don't have one)",
            pick(
                format!("Move with {}. Walk a few metres.", c.move_keys),
                "Drag on the left half of the screen to walk.",
            ),
            "Please complete mandatory training. Completion is tracked. Learning is optional.",
        ),
        Step::Camera => (
            "look around",
            pick(
                "Hold the left or right mouse button and drag to look around. Wheel zooms.".into(),
                "Drag on the right half of the screen to look around. Pinch to zoom.",
            ),
            "Everything you see was approved by a committee that has never been outside.",
        ),
        Step::Accept => (
            "take a ticket",
            pick(
                format!(
                    "Walk to the HR Onboarding Bot (the yellow ! by the parking lot), press {} or right-click it and accept the ticket.",
                    c.interact
                ),
                "Walk to the HR Onboarding Bot (the yellow ! by the parking lot), tap F to talk and accept the ticket.",
            ),
            "Every task needs a ticket. Every ticket needs a task. Nobody knows which came first.",
        ),
        Step::Target => (
            "pick a stakeholder",
            pick(
                format!(
                    "Press {} or click the Mandatory Training Dummy to target it.",
                    c.target
                ),
                "Tap the Mandatory Training Dummy to target it.",
            ),
            "In this company 'stakeholder' means the thing you are about to hit.",
        ),
        Step::AutoAttack => (
            "auto attack",
            pick(
                format!("Press {} to start auto attacking.", c.auto),
                "Tap the T button (auto attack) on the right.",
            ),
            "The only automation here that works as advertised.",
        ),
        Step::Spell1 => (
            "cast your first ability",
            pick(
                format!("Cast {} with {}.", c.spell1.0, c.spell1.1),
                &format!("Tap {} on the action bar.", c.spell1.0),
            ),
            "The AI says it could do this. The AI says a lot of things.",
        ),
        Step::Spell2 => (
            "cast your second ability",
            pick(
                format!("Cast {} with {}.", c.spell2.0, c.spell2.1),
                &format!("Tap {} on the action bar.", c.spell2.0),
            ),
            "Twice the output, same salary. Welcome to scaling.",
        ),
        Step::Interrupt => match (&c.interrupt, t.can_interrupt) {
            (Some((name, key, _)), true) => (
                "interrupt the training video",
                pick(
                    format!(
                        "When the dummy's red cast bar says Mandatory Training Video, press {key} ({name}) to interrupt it."
                    ),
                    &format!("When the red cast bar says Mandatory Training Video, tap {name}."),
                ),
                "Interrupting in meetings is a career risk. Here it is a skill.",
            ),
            (Some((name, key, level)), false) => (
                "survive the training video",
                format!(
                    "A red cast bar is a must-interrupt. Yours is {name} ({key}), unlocked at level {level}. Until then, watch the dummy's bar and sit through Mandatory Training Video."
                ),
                "Do what everyone does: video on, sound off, nod at the end.",
            ),
            (None, _) => (
                "survive the training video",
                "A red cast bar is a must-interrupt. Watch the dummy's bar and sit through Mandatory Training Video.".into(),
                "Do what everyone does: video on, sound off, nod at the end.",
            ),
        },
        Step::RoleTip => match t.role {
            Role::Tank => (
                "tank tip: soak it",
                format!(
                    "Cast {} ({}) and let the dummy hit you while it holds. {}",
                    c.role_spell.0,
                    c.role_spell.1,
                    c.taunt
                        .as_ref()
                        .map_or(String::new(), |(n, l)| format!(
                            "Taunt ({n}) arrives at level {l}: it pulls hits off your colleagues."
                        ))
                ),
                "Tanks get hit on purpose. HR calls it 'resilience' and gives it to you in writing.",
            ),
            Role::Healer => (
                "healer tip: heal a colleague",
                pick(
                    format!(
                        "Click the Burnt-Out Colleague (by the HR bot) to target them, then cast {} ({}).",
                        c.role_spell.0, c.role_spell.1
                    ),
                    &format!(
                        "Tap the Burnt-Out Colleague by the HR bot, then tap {}.",
                        c.role_spell.0
                    ),
                ),
                "Nobody else will heal them. It is not in anyone's OKRs.",
            ),
            Role::Melee => (
                "melee tip: positioning",
                "Stay on the dummy, but when a Compliance Zone circle appears under you, step out before it lands.".into(),
                "Melee lives in the blast radius. The trick is leaving it before the invite lands.",
            ),
            Role::Ranged => (
                "ranged tip: kiting",
                format!(
                    "Back off to {KITE_M:.0}+ m while it shuffles after you, then hit it with {}.{}",
                    c.spell1.0,
                    c.class_tip
                        .as_ref()
                        .map_or(String::new(), |t| format!(" {t}"))
                ),
                "Distance is the only boundary management respects.",
            ),
        },
        Step::Loot => (
            "loot the corpse",
            pick(
                format!(
                    "Finish the dummy, then press {} or right-click its corpse to loot it.",
                    c.interact
                ),
                "Finish the dummy, then walk to its corpse and tap F to loot it.",
            ),
            "In the AI economy, taking things off corpses is called 'data acquisition'.",
        ),
        Step::TurnIn => (
            "close the ticket",
            pick(
                format!(
                    "Back to the HR Onboarding Bot: press {} to turn the ticket in. The tracker on the right shows it is ready.",
                    c.interact
                ),
                "Back to the HR Onboarding Bot: tap F to turn the ticket in. The tracker shows it is ready.",
            ),
            "It will be marked done, then reopened next quarter as a 'learning'.",
        ),
        Step::Windows => (
            "find your windows",
            {
                let left: Vec<String> = t
                    .windows_left()
                    .into_iter()
                    .map(|w| match (w, c.touch) {
                        (Window::Map, false) => format!("map ({})", c.map),
                        (Window::Sheet, false) => format!("gear ({})", c.sheet),
                        (Window::Career, false) => format!("career ({})", c.career),
                        (Window::Map, true) => "map (Menu)".into(),
                        (Window::Sheet, true) => "gear (Menu)".into(),
                        (Window::Career, true) => "career (Menu)".into(),
                    })
                    .collect();
                format!("Open each once: {}.", left.join(", "))
            },
            "Three windows. Your manager has four monitors and reads none of them.",
        ),
        Step::Scooter => (
            "commute like an executive",
            pick(
                format!("Press {} to ride an e-scooter.", c.scooter),
                "Tap More, then Ride, to take an e-scooter.",
            ),
            "Faster than the elevator, less judgmental than the people who park them on fire exits.",
        ),
        Step::Handoff => (
            "meet your buddy",
            pick(
                format!(
                    "Talk to the AI-Native Intern ({}) by your car. He has your first real ticket, and the way to the campus.",
                    c.interact
                ),
                "Walk to the AI-Native Intern by your car and tap F. He has your first real ticket, and the way to the campus.",
            ),
            "He has been here two days. That makes him senior.",
        ),
    };
    Some(Copy { title, how, quip })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The happy path through every step, as the glue would report it.
    fn run(role: Role, role_signal: Signal, can_interrupt: bool) -> Tutorial {
        let mut t = Tutorial::new(role, can_interrupt);
        let video = Signal::VideoEnded {
            interrupted: can_interrupt,
        };
        for s in [
            Signal::Moved(MOVE_M + 0.5),
            Signal::CameraMoved,
            Signal::QuestAccepted,
            Signal::TargetedDummy,
            Signal::AutoAttackOn,
            Signal::CastSlot(0),
            Signal::CastSlot(1),
            video,
            role_signal,
            Signal::LootedDummy,
            Signal::QuestTurnedIn,
            Signal::Opened(Window::Map),
            Signal::Opened(Window::Sheet),
            Signal::Opened(Window::Career),
            Signal::Mounted,
            Signal::TalkedToIntern,
        ] {
            t.feed(s);
        }
        t
    }

    #[test]
    fn every_role_finishes_with_its_own_tip() {
        for (role, sig) in [
            (Role::Tank, Signal::Soaked),
            (Role::Healer, Signal::HealedColleague),
            (Role::Melee, Signal::DodgedZone),
            (Role::Ranged, Signal::RangedHit(KITE_M + 1.0)),
        ] {
            assert!(run(role, sig, false).done(), "{role:?}");
            assert!(run(role, sig, true).done(), "{role:?}");
        }
    }

    #[test]
    fn another_roles_tip_does_not_count() {
        let t = run(Role::Healer, Signal::Soaked, false);
        assert_eq!(t.current(), Some(Step::RoleTip));
        let t = run(Role::Ranged, Signal::RangedHit(KITE_M - 3.0), false);
        assert_eq!(t.current(), Some(Step::RoleTip));
    }

    #[test]
    fn steps_only_advance_on_their_own_signal() {
        let mut t = Tutorial::new(Role::Melee, false);
        assert!(!t.feed(Signal::Moved(1.0)));
        assert!(!t.feed(Signal::CastSlot(0)));
        assert!(!t.feed(Signal::Mounted));
        assert_eq!(t.current(), Some(Step::Move));
        assert!(t.feed(Signal::Moved(MOVE_M)));
        assert_eq!(t.current(), Some(Step::Camera));
        // Slot 2 before slot 1 does not skip anything.
        t.step = 5;
        assert!(!t.feed(Signal::CastSlot(1)));
        assert!(t.feed(Signal::CastSlot(0)));
    }

    #[test]
    fn interrupt_is_required_once_it_is_known() {
        let mut t = Tutorial::new(Role::Ranged, true);
        t.step = 7;
        assert!(!t.feed(Signal::VideoEnded { interrupted: false }));
        assert!(t.feed(Signal::VideoEnded { interrupted: true }));
        let mut fresh = Tutorial::new(Role::Ranged, false);
        fresh.step = 7;
        assert!(fresh.feed(Signal::VideoEnded { interrupted: false }));
    }

    #[test]
    fn windows_step_wants_all_three_in_any_order() {
        let mut t = Tutorial::new(Role::Tank, false);
        t.step = 11;
        assert!(!t.feed(Signal::Opened(Window::Career)));
        assert!(!t.feed(Signal::Opened(Window::Career)));
        assert_eq!(t.windows_left(), vec![Window::Map, Window::Sheet]);
        assert!(!t.feed(Signal::Opened(Window::Map)));
        assert!(t.feed(Signal::Opened(Window::Sheet)));
        assert_eq!(t.current(), Some(Step::Scooter));
    }

    #[test]
    fn every_step_has_copy_for_desktop_and_touch() {
        for touch in [false, true] {
            let c = Ctx {
                touch,
                move_keys: "WASD".into(),
                target: "Tab".into(),
                auto: "F".into(),
                interact: "E".into(),
                map: "M".into(),
                sheet: "C".into(),
                career: "N".into(),
                scooter: "Y".into(),
                spell1: ("kill -9".into(), "1".into()),
                spell2: ("Hotfix".into(), "2".into()),
                interrupt: Some(("SIGSTOP".into(), "3".into(), 2)),
                role_spell: ("Hotfix".into(), "2".into()),
                taunt: None,
                class_tip: Some("File a Bug Report (2) first.".into()),
            };
            for role in [Role::Tank, Role::Healer, Role::Melee, Role::Ranged] {
                let mut t = Tutorial::new(role, false);
                for k in 0..STEPS.len() {
                    t.step = k;
                    let Some(copy) = copy(&t, &c) else {
                        panic!("no copy for step {k}");
                    };
                    assert!(!copy.how.is_empty() && !copy.quip.is_empty());
                    if role == Role::Ranged && t.current() == Some(Step::RoleTip) {
                        assert!(copy.how.contains("Bug Report"), "{}", copy.how);
                    }
                    // Touch copy never asks for a key.
                    if touch && !matches!(t.current(), Some(Step::Interrupt | Step::RoleTip)) {
                        assert!(!copy.how.contains("press "), "{}", copy.how);
                    }
                }
                t.step = STEPS.len();
                assert!(copy(&t, &c).is_none());
            }
        }
    }
}
