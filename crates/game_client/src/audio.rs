//! Music and sound effects.
//!
//! Gameplay code never plays sounds itself. This module listens to the same sim and
//! presentation events as the HUD and maps them through the data tables below
//! (`TABLE` for clips, `SPELL_SFX` for spell ids, `TRACKS` for loops).
//!
//! Browsers keep audio suspended until the first user gesture, so nothing is played
//! (or queued) before one; `web/index.html` resumes the WebAudio context on the same gesture.
// Bevy system signatures are inherently wide; splitting them would only obscure data flow.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::combat::{CombatEvent, SimRead, SimReady, SimView};
use crate::game::GameFlow;
use crate::hud::{KEY_RIM, KEY_TOP, KEY_TOP_HI, label, set_text, vgrad};
use crate::input::{Paused, TrophiesOpen};
use crate::presentation::PresentEvent;
use bevy::audio::Volume;
use bevy::input::touch::Touches;
use bevy::prelude::*;
use combat_core::{
    CastError, Content, Effect, EntityId, Event, Medal, PropKind, School, SpellId, distance,
};
use std::collections::HashMap;

/// Sound effects farther than this from the player are inaudible.
const HEARD_M: f32 = 32.0;
const CROSSFADE_SECS: f32 = 1.5;
/// Random pitch change per play, as a fraction either way.
const PITCH_JITTER: f32 = 0.05;
const MAX_VOICES: usize = 32;
const MIN_GAIN: f32 = 0.02;
const VOLUME_STEP: f32 = 0.1;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Snd {
    Key,
    Error,
    Loot,
    TrophyOpen,
    TrophyClose,
    LevelUp,
    SpellLearned,
    FloorCleared,
    ElevatorDing,
    ElevatorDoor,
    Victory,
    Death,
    CastHum,
    CastEnemy,
    CodeReview,
    Refactor,
    Guardrails,
    RejectPr,
    GitBlame,
    Rollback,
    Caffeine,
    CoffeeBreak,
    RubberDuck,
    Postmortem,
    WriteByHand,
    HitLight,
    HitSpell,
    HitCrit,
    Tick,
    Miss,
    HitPlayer,
    Heal,
    Stun,
    Summon,
    UiClick,
    UiHover,
    WindowOpen,
    WindowClose,
    QuestAccept,
    QuestDone,
    Kudos,
    Promotion,
    Interrupt,
    Telegraph,
    Block,
    Scooter,
    Shuttle,
    CoffeeMachine,
    Printer,
    Vending,
    MedalBronze,
    MedalSilver,
    MedalGold,
    CastArcane,
    CastPhysical,
    VoiceLevelUp,
    VoiceGameOver,
    VoiceFlawless,
}

struct Def {
    snd: Snd,
    /// One is picked at random per play.
    files: &'static [&'static str],
    vol: f32,
    /// Minimum seconds between two plays, so mass events do not stack.
    cooldown: f32,
    /// Pitched jingles and chimes keep their tuning.
    jitter: bool,
}

const fn def(
    snd: Snd,
    files: &'static [&'static str],
    vol: f32,
    cooldown: f32,
    jitter: bool,
) -> Def {
    Def {
        snd,
        files,
        vol,
        cooldown,
        jitter,
    }
}

static TABLE: &[Def] = &[
    def(
        Snd::Key,
        &[
            "audio/sfx/key_1.ogg",
            "audio/sfx/key_2.ogg",
            "audio/sfx/key_3.ogg",
        ],
        0.5,
        0.04,
        true,
    ),
    def(Snd::Error, &["audio/sfx/error.ogg"], 0.45, 0.5, true),
    def(Snd::Loot, &["audio/sfx/loot.ogg"], 0.6, 0.15, true),
    def(
        Snd::TrophyOpen,
        &["audio/sfx/trophy_open.ogg"],
        0.6,
        0.2,
        true,
    ),
    def(
        Snd::TrophyClose,
        &["audio/sfx/trophy_close.ogg"],
        0.5,
        0.2,
        true,
    ),
    def(Snd::LevelUp, &["audio/sfx/level_up.ogg"], 0.7, 1.0, false),
    def(
        Snd::SpellLearned,
        &["audio/sfx/spell_learned.ogg"],
        0.6,
        0.5,
        false,
    ),
    def(
        Snd::FloorCleared,
        &["audio/sfx/floor_cleared.ogg"],
        0.6,
        1.0,
        false,
    ),
    def(
        Snd::ElevatorDing,
        &["audio/sfx/elevator_ding.ogg"],
        0.7,
        0.5,
        false,
    ),
    def(
        Snd::ElevatorDoor,
        &["audio/sfx/elevator_door.ogg"],
        0.5,
        0.5,
        true,
    ),
    def(Snd::Victory, &["audio/sfx/victory.ogg"], 0.8, 2.0, false),
    def(Snd::Death, &["audio/sfx/death.ogg"], 0.7, 1.0, true),
    def(Snd::CastHum, &["audio/sfx/cast_hum.ogg"], 0.18, 0.3, true),
    def(
        Snd::CastEnemy,
        &["audio/sfx/cast_enemy.ogg"],
        0.4,
        0.25,
        true,
    ),
    def(
        Snd::CodeReview,
        &["audio/sfx/cast_code_review.ogg"],
        0.45,
        0.1,
        true,
    ),
    def(
        Snd::Refactor,
        &["audio/sfx/cast_refactor.ogg"],
        0.45,
        0.1,
        true,
    ),
    def(
        Snd::Guardrails,
        &["audio/sfx/cast_guardrails.ogg"],
        0.5,
        0.1,
        true,
    ),
    def(
        Snd::RejectPr,
        &["audio/sfx/cast_reject_pr.ogg"],
        0.7,
        0.1,
        true,
    ),
    def(
        Snd::GitBlame,
        &["audio/sfx/cast_git_blame.ogg"],
        0.4,
        0.1,
        true,
    ),
    def(
        Snd::Rollback,
        &["audio/sfx/cast_rollback.ogg"],
        0.5,
        0.1,
        true,
    ),
    def(
        Snd::Caffeine,
        &["audio/sfx/cast_caffeine.ogg"],
        0.6,
        0.1,
        true,
    ),
    def(
        Snd::CoffeeBreak,
        &["audio/sfx/cast_coffee_break.ogg"],
        0.6,
        0.5,
        true,
    ),
    def(
        Snd::RubberDuck,
        &["audio/sfx/cast_rubber_duck.ogg"],
        0.6,
        0.1,
        true,
    ),
    def(
        Snd::Postmortem,
        &["audio/sfx/cast_postmortem.ogg"],
        0.8,
        0.1,
        true,
    ),
    def(
        Snd::WriteByHand,
        &["audio/sfx/cast_write_by_hand.ogg"],
        0.6,
        0.1,
        true,
    ),
    def(
        Snd::HitLight,
        &[
            "audio/sfx/hit_light_1.ogg",
            "audio/sfx/hit_light_2.ogg",
            "audio/sfx/hit_light_3.ogg",
        ],
        0.45,
        0.08,
        true,
    ),
    def(
        Snd::HitSpell,
        &["audio/sfx/hit_spell_1.ogg", "audio/sfx/hit_spell_2.ogg"],
        0.5,
        0.1,
        true,
    ),
    def(Snd::HitCrit, &["audio/sfx/hit_crit.ogg"], 0.3, 0.3, true),
    def(Snd::Tick, &["audio/sfx/tick.ogg"], 0.5, 0.15, true),
    def(Snd::Miss, &["audio/sfx/miss.ogg"], 0.4, 0.15, true),
    def(
        Snd::HitPlayer,
        &["audio/sfx/hit_player_1.ogg", "audio/sfx/hit_player_2.ogg"],
        0.6,
        0.15,
        true,
    ),
    def(Snd::Heal, &["audio/sfx/heal.ogg"], 0.45, 0.3, true),
    def(Snd::Stun, &["audio/sfx/stun.ogg"], 0.6, 0.3, true),
    def(Snd::Summon, &["audio/sfx/summon.ogg"], 0.5, 0.1, true),
    def(Snd::UiClick, &["audio/sfx/ui_click.ogg"], 0.4, 0.05, true),
    // Quiet and rate-limited: a sweep across a button row must not machine-gun.
    def(Snd::UiHover, &["audio/sfx/ui_hover.ogg"], 0.12, 0.08, true),
    def(
        Snd::WindowOpen,
        &["audio/sfx/window_open.ogg"],
        0.4,
        0.2,
        true,
    ),
    def(
        Snd::WindowClose,
        &["audio/sfx/window_close.ogg"],
        0.35,
        0.2,
        true,
    ),
    def(
        Snd::QuestAccept,
        &["audio/sfx/quest_accept.ogg"],
        0.5,
        0.3,
        false,
    ),
    def(
        Snd::QuestDone,
        &["audio/sfx/quest_done.ogg"],
        0.55,
        0.3,
        false,
    ),
    def(Snd::Kudos, &["audio/sfx/kudos.ogg"], 0.35, 0.4, false),
    def(
        Snd::Promotion,
        &["audio/sfx/promotion.ogg"],
        0.55,
        0.5,
        false,
    ),
    def(
        Snd::Interrupt,
        &["audio/sfx/interrupt.ogg"],
        0.55,
        0.2,
        true,
    ),
    def(
        Snd::Telegraph,
        &["audio/sfx/telegraph.ogg"],
        0.35,
        0.6,
        false,
    ),
    def(Snd::Block, &["audio/sfx/block.ogg"], 0.5, 0.15, true),
    def(Snd::Scooter, &["audio/sfx/scooter.ogg"], 0.4, 0.5, true),
    def(Snd::Shuttle, &["audio/sfx/shuttle.ogg"], 0.45, 0.5, true),
    def(
        Snd::CoffeeMachine,
        &["audio/sfx/coffee_machine.ogg"],
        0.45,
        0.5,
        true,
    ),
    def(Snd::Printer, &["audio/sfx/printer.ogg"], 0.35, 0.5, true),
    def(Snd::Vending, &["audio/sfx/vending.ogg"], 0.5, 0.3, true),
    def(
        Snd::MedalBronze,
        &["audio/sfx/medal_bronze.ogg"],
        0.55,
        0.5,
        false,
    ),
    def(
        Snd::MedalSilver,
        &["audio/sfx/medal_silver.ogg"],
        0.55,
        0.5,
        false,
    ),
    def(
        Snd::MedalGold,
        &["audio/sfx/medal_gold.ogg"],
        0.6,
        0.5,
        false,
    ),
    def(
        Snd::CastArcane,
        &["audio/sfx/cast_arcane.ogg"],
        0.4,
        0.1,
        true,
    ),
    def(
        Snd::CastPhysical,
        &["audio/sfx/cast_physical.ogg"],
        0.4,
        0.1,
        true,
    ),
    // Voice lines are rare punchlines, so they sit a little under the jingles.
    def(
        Snd::VoiceLevelUp,
        &["audio/sfx/voice_level_up.ogg"],
        0.5,
        2.0,
        false,
    ),
    def(
        Snd::VoiceGameOver,
        &["audio/sfx/voice_game_over.ogg"],
        0.55,
        2.0,
        false,
    ),
    def(
        Snd::VoiceFlawless,
        &["audio/sfx/voice_flawless.ogg"],
        0.55,
        2.0,
        false,
    ),
];

struct SpellSfx {
    id: u32,
    /// Checked against content in a test, so a renumbered spell cannot silently go quiet.
    #[cfg_attr(not(test), allow(dead_code))]
    name: &'static str,
    snd: Snd,
    /// Channels play when they start; everything else when the cast completes.
    on_start: bool,
}

const fn spell(id: u32, name: &'static str, snd: Snd, on_start: bool) -> SpellSfx {
    SpellSfx {
        id,
        name,
        snd,
        on_start,
    }
}

const SPELL_SFX: &[SpellSfx] = &[
    spell(1, "Code Review", Snd::CodeReview, false),
    spell(2, "Refactor", Snd::Refactor, false),
    spell(3, "Guardrails", Snd::Guardrails, false),
    spell(4, "Reject PR", Snd::RejectPr, false),
    spell(6, "git blame", Snd::GitBlame, false),
    spell(7, "Rollback", Snd::Rollback, false),
    spell(8, "On-Call Caffeine", Snd::Caffeine, false),
    spell(9, "Rubber Duck", Snd::RubberDuck, false),
    spell(10, "Postmortem", Snd::Postmortem, false),
    spell(11, "Write It By Hand", Snd::WriteByHand, false),
    spell(12, "Coffee Break", Snd::CoffeeBreak, true),
];

fn spell_sfx(id: SpellId) -> Option<&'static SpellSfx> {
    SPELL_SFX.iter().find(|s| s.id == id.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Track {
    Explore,
    Boss,
    Ambient,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Group {
    Master,
    Music,
    Sfx,
}

struct TrackDef {
    track: Track,
    level: f32,
    group: Group,
}

const TRACKS: &[TrackDef] = &[
    TrackDef {
        track: Track::Explore,
        level: 0.4,
        group: Group::Music,
    },
    TrackDef {
        track: Track::Boss,
        level: 0.45,
        group: Group::Music,
    },
    // Air-con hum: a bed, not a feature.
    TrackDef {
        track: Track::Ambient,
        level: 0.12,
        group: Group::Sfx,
    },
];

/// The loop for `track` in `zone`: one per floor or raid wing (docs/MUSIC.md), the night loop
/// outdoors after dark, raid finales their own boss track. Loaded lazily, so the web build only
/// downloads the music of zones the player visits.
fn music_file(track: Track, zone: &str, outdoor_night: bool) -> &'static str {
    match track {
        Track::Ambient => "audio/sfx/ambient_aircon.ogg",
        Track::Boss => match zone {
            // Quarterly Business Review: the corporate lobby ambient all the way down (bosses use the generic boss loop).
            "qbr/1" | "qbr/2" | "qbr/3" | "qbr/4" => "audio/music/office_muzak.ogg",
            "offsite/4" => "audio/music/raid_boss_offsite.ogg",
            "vibecon/5" => "audio/music/raid_boss_vibecon.ogg",
            _ => "audio/music/boss_battle.ogg",
        },
        Track::Explore if outdoor_night => "audio/music/night.ogg",
        Track::Explore => match zone {
            "campus" | "offsite/3" => "audio/music/campus.ogg",
            "parking_p3" => "audio/music/parking_p3.ogg",
            "tower/1" | "client/1" | "openspace/1" => "audio/music/open_space.ogg",
            "tower/3" | "teambuilding/1" => "audio/music/sales_marketing.ogg",
            "tower/4" | "review/1" => "audio/music/pmo.ogg",
            "tower/5" => "audio/music/c_level_penthouse.ogg",
            "datacenter/1" | "migration/1" => "audio/music/datacenter_hall_a.ogg",
            "datacenter/2" => "audio/music/datacenter_hall_b.ogg",
            "lab/1" => "audio/music/lab_showroom.ogg",
            "lab/2" | "reorg/1" => "audio/music/lab_rack_room.ogg",
            "home/1" => "audio/music/home.ogg",
            "offsite/1" | "offsite/2" => "audio/music/offsite_lobby.ogg",
            // The Onboarding: elevator muzak all the way down (its boss uses the generic boss loop).
            "onboarding/1" | "onboarding/2" | "onboarding/3" => "audio/music/office_muzak.ogg",
            // The Migration: the Hall B drones all the way down (its bosses use the generic boss loop).
            "basement/1" | "basement/2" | "basement/3" | "basement/4" => {
                "audio/music/datacenter_hall_b.ogg"
            }
            "offsite/4" => "audio/music/raid_boss_offsite.ogg",
            "vibecon/1" | "vibecon/2" | "vibecon/3" | "hackathon/1" => {
                "audio/music/vibecon_expo.ogg"
            }
            "vibecon/4" => "audio/music/vibecon_afterparty.ogg",
            "vibecon/5" => "audio/music/raid_boss_vibecon.ogg",
            _ => "audio/music/office_muzak.ogg",
        },
    }
}

#[derive(Resource)]
struct AudioSettings {
    master: f32,
    music: f32,
    sfx: f32,
    muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 0.6,
            sfx: 0.8,
            muted: start_muted(),
        }
    }
}

/// Test runs start silent: `WOW_MUTE=1` natively, `?mute=1` in the page URL on the web. The web
/// build also remembers the viewer's last choice (localStorage) and mirrors it into the URL.
#[cfg(not(target_arch = "wasm32"))]
fn start_muted() -> bool {
    std::env::var_os("WOW_MUTE").is_some()
}

#[cfg(target_arch = "wasm32")]
fn start_muted() -> bool {
    let query = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    let muted = if query.contains("mute=1") {
        true
    } else if query.contains("mute=0") {
        false
    } else {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(MUTE_KEY).ok().flatten())
            .is_some_and(|v| v == "1")
    };
    remember_mute(muted);
    muted
}

#[cfg(target_arch = "wasm32")]
const MUTE_KEY: &str = "tld.muted";

/// Persists the mute choice and keeps `?mute=1` in the address bar while muted, so a reload or
/// a shared link starts the same way.
#[cfg(target_arch = "wasm32")]
fn remember_mute(muted: bool) {
    let Some(w) = web_sys::window() else { return };
    if let Ok(Some(s)) = w.local_storage() {
        let _ = s.set_item(MUTE_KEY, if muted { "1" } else { "0" });
    }
    let loc = w.location();
    let (Ok(path), Ok(search), Ok(hash)) = (loc.pathname(), loc.search(), loc.hash()) else {
        return;
    };
    let mut params: Vec<&str> = search
        .trim_start_matches('?')
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("mute="))
        .collect();
    if muted {
        params.push("mute=1");
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };
    if query != search
        && let Ok(h) = w.history()
    {
        let _ = h.replace_state_with_url(
            &wasm_bindgen::JsValue::NULL,
            "",
            Some(&format!("{path}{query}{hash}")),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn remember_mute(_muted: bool) {}

impl AudioSettings {
    /// The slider's own position, as shown in the pause panel.
    fn slider(&self, g: Group) -> f32 {
        match g {
            Group::Master => self.master,
            Group::Music => self.music,
            Group::Sfx => self.sfx,
        }
    }

    fn group(&self, g: Group) -> f32 {
        match g {
            Group::Master => self.master,
            Group::Music => self.master * self.music,
            Group::Sfx => self.master * self.sfx,
        }
    }

    fn effective(&self, g: Group) -> f32 {
        if self.muted { 0.0 } else { self.group(g) }
    }

    fn step(&mut self, g: Group, delta: f32) {
        let slot = match g {
            Group::Master => &mut self.master,
            Group::Music => &mut self.music,
            Group::Sfx => &mut self.sfx,
        };
        // Rounded so repeated steps never drift off the 10% grid.
        *slot = (((*slot + delta) * 10.0).round() / 10.0).clamp(0.0, 1.0);
    }
}

/// Browsers refuse audio before a gesture; native has no such rule.
#[derive(Resource)]
struct Unlocked(bool);

/// Request to play one sound; `delay` lets event handlers stage multi-part cues.
#[derive(Message, Clone, Copy)]
struct Sfx {
    snd: Snd,
    gain: f32,
    delay: f32,
}

impl Sfx {
    fn new(snd: Snd) -> Self {
        Self {
            snd,
            gain: 1.0,
            delay: 0.0,
        }
    }

    fn gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    fn after(mut self, secs: f32) -> Self {
        self.delay = secs;
        self
    }
}

#[derive(Resource)]
struct Bank {
    clips: HashMap<Snd, Vec<Handle<AudioSource>>>,
    last_played: HashMap<Snd, f32>,
    pending: Vec<Sfx>,
    rng: u64,
}

impl Bank {
    /// xorshift64: audio variation needs no quality and the game has no `rand` dependency.
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[derive(Component)]
struct SfxVoice;

#[derive(Component)]
struct MusicVoice {
    track: Track,
    file: &'static str,
    fade: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mood {
    /// Victory screen: the jingle has the stage.
    Silent,
    Explore,
    Boss,
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioSettings>()
            .insert_resource(Unlocked(!cfg!(target_arch = "wasm32")))
            .add_message::<Sfx>()
            .add_systems(Startup, (load_bank, spawn_panel))
            .add_systems(
                Update,
                (
                    unlock_on_gesture,
                    toggle_mute,
                    ui_sfx,
                    combat_sfx.in_set(SimReady),
                    drive_music.in_set(SimReady),
                    play_sfx,
                    audio_panel,
                )
                    .chain(),
            );
    }
}

fn load_bank(mut commands: Commands, assets: Res<AssetServer>) {
    let clips = TABLE
        .iter()
        .map(|d| (d.snd, d.files.iter().map(|f| assets.load(*f)).collect()))
        .collect();
    commands.insert_resource(Bank {
        clips,
        last_played: HashMap::new(),
        pending: Vec::new(),
        rng: 0x9E37_79B9_7F4A_7C15,
    });
}

fn unlock_on_gesture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut unlocked: ResMut<Unlocked>,
) {
    if !unlocked.0
        && (keys.get_just_pressed().next().is_some()
            || mouse.get_just_pressed().next().is_some()
            || touches.any_just_pressed())
    {
        unlocked.0 = true;
    }
}

fn toggle_mute(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    mut settings: ResMut<AudioSettings>,
) {
    if binds.just(&keys, crate::keybinds::Action::Mute) {
        settings.muted = !settings.muted;
        remember_mute(settings.muted);
    }
}

/// The coffee key and every action bar slot key, number row and numpad.
fn action_pressed(binds: &crate::keybinds::Keybinds, keys: &ButtonInput<KeyCode>) -> bool {
    keys.just_pressed(binds.key(crate::keybinds::Action::Coffee))
        || (0..crate::combat::SLOT_COUNT).any(|i| keys.any_just_pressed(binds.slot_keys(i)))
}

/// Interface sounds that have no sim event: keycaps, the trophy window, the victory screen.
fn ui_sfx(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<crate::keybinds::Keybinds>,
    paused: Res<Paused>,
    flow: Res<GameFlow>,
    trophies: Res<TrophiesOpen>,
    buttons: Query<&Interaction, (With<Button>, Changed<Interaction>)>,
    mut was_open: Local<bool>,
    mut was_victory: Local<bool>,
    mut was_paused: Local<bool>,
    mut out: MessageWriter<Sfx>,
) {
    for i in &buttons {
        match i {
            Interaction::Pressed => {
                out.write(Sfx::new(Snd::UiClick));
            }
            Interaction::Hovered => {
                out.write(Sfx::new(Snd::UiHover));
            }
            Interaction::None => {}
        }
    }
    if paused.0 != *was_paused {
        *was_paused = paused.0;
        out.write(Sfx::new(if paused.0 {
            Snd::WindowOpen
        } else {
            Snd::WindowClose
        }));
    }
    if !paused.0 && !flow.input_locked() && action_pressed(&binds, &keys) {
        out.write(Sfx::new(Snd::Key));
    }
    if trophies.0 != *was_open {
        *was_open = trophies.0;
        out.write(Sfx::new(if trophies.0 {
            Snd::TrophyOpen
        } else {
            Snd::TrophyClose
        }));
    }
    if flow.victory && !*was_victory {
        out.write(Sfx::new(Snd::Victory));
    }
    *was_victory = flow.victory;
}

fn school_cast(school: School) -> Snd {
    match school {
        School::Arcane => Snd::CastArcane,
        School::Physical => Snd::CastPhysical,
    }
}

/// Sound of using a prop; props without a distinct one stay silent beyond their chat line.
fn prop_sfx(kind: PropKind) -> Option<Snd> {
    match kind {
        PropKind::Coffee => Some(Snd::CoffeeMachine),
        PropKind::Printer => Some(Snd::Printer),
        PropKind::Vending => Some(Snd::Vending),
        PropKind::Scooter => Some(Snd::Scooter),
        PropKind::ShuttleStop => Some(Snd::Shuttle),
        _ => None,
    }
}

/// 1 at the player, fading to 0 at `HEARD_M`; quadratic so far sounds die off quickly.
fn falloff(sim: &dyn SimRead, player: EntityId, unit: EntityId) -> f32 {
    if unit == player {
        return 1.0;
    }
    match (sim.unit(player), sim.unit(unit)) {
        (Some(p), Some(u)) => (1.0 - distance(p.pos, u.pos) / HEARD_M)
            .clamp(0.0, 1.0)
            .powi(2),
        _ => 0.0,
    }
}

/// Mirrors `floating_text::is_periodic`: damage from a spell with no direct damage effect is a DoT tick.
fn is_periodic(content: &Content, spell: Option<SpellId>) -> bool {
    spell.and_then(|id| content.spell(id)).is_some_and(|d| {
        !std::iter::once(&d.effect)
            .chain(&d.extra_effects)
            .any(|e| matches!(e, Effect::Damage { .. }))
    })
}

fn combat_sfx(
    mut events: MessageReader<CombatEvent>,
    mut present: MessageReader<PresentEvent>,
    sim: SimView,
    mut out: MessageWriter<Sfx>,
    mut first_floor: Local<bool>,
    mut sp_seen: Local<Option<u32>>,
) {
    let sim = &*sim;
    let Some(player) = sim.player() else {
        return;
    };
    let near = |unit| falloff(sim, player, unit);
    for CombatEvent(ev) in events.read() {
        match ev {
            Event::CastStarted { caster, spell, .. } if *caster == player => {
                out.write(match spell_sfx(*spell).filter(|s| s.on_start) {
                    Some(s) => Sfx::new(s.snd),
                    None => Sfx::new(Snd::CastHum),
                });
            }
            Event::CastStarted { caster, .. } => {
                out.write(Sfx::new(Snd::CastHum).gain(near(*caster)));
            }
            Event::CastSucceeded { caster, spell, .. } if *caster == player => {
                match spell_sfx(*spell) {
                    Some(s) if !s.on_start => {
                        out.write(Sfx::new(s.snd));
                    }
                    Some(_) => {}
                    // Spells without their own clip get one per school.
                    None => {
                        if let Some(d) = sim.content().spell(*spell) {
                            out.write(Sfx::new(school_cast(d.school)));
                        }
                    }
                }
            }
            Event::CastSucceeded { caster, .. } => {
                out.write(Sfx::new(Snd::CastEnemy).gain(near(*caster)));
            }
            // Queued spells retry every tick during the GCD; that is not an error worth a buzz.
            Event::CastFailed { caster, reason, .. }
                if *caster == player && *reason != CastError::OnGcd =>
            {
                out.write(Sfx::new(Snd::Error));
            }
            Event::CastInterrupted { caster, .. } => {
                out.write(Sfx::new(Snd::Interrupt).gain(near(*caster)));
            }
            Event::TelegraphStarted { source, .. } => {
                out.write(Sfx::new(Snd::Telegraph).gain(near(*source).max(0.5)));
            }
            Event::MeleeSwing { attacker, .. } if *attacker == player => {
                out.write(Sfx::new(Snd::Tick));
            }
            Event::Heal { source, target, .. } if *source == player || *target == player => {
                out.write(Sfx::new(Snd::Heal));
            }
            Event::AuraApplied { target, key }
                if sim.content().aura(key).is_some_and(|a| a.stun) =>
            {
                out.write(Sfx::new(Snd::Stun).gain(near(*target)));
            }
            Event::Summoned { unit, .. } => {
                out.write(Sfx::new(Snd::Summon).gain(near(*unit)));
            }
            Event::Death { unit, .. } if *unit == player => {
                out.write(Sfx::new(Snd::Death));
                out.write(Sfx::new(Snd::VoiceGameOver).after(0.9));
            }
            Event::Looted { looter, .. } if *looter == player => {
                out.write(Sfx::new(Snd::Loot));
            }
            Event::LevelUp { unit, .. } if *unit == player => {
                out.write(Sfx::new(Snd::LevelUp));
                out.write(Sfx::new(Snd::VoiceLevelUp).after(0.7));
            }
            Event::TalentsChanged { unit, .. } if *unit == player => {
                out.write(Sfx::new(Snd::Promotion));
            }
            Event::QuestAccepted { .. } => {
                out.write(Sfx::new(Snd::QuestAccept));
            }
            Event::Bag { unit, sp, .. } if *unit == player => {
                // The first Bag is the opening balance, not a gain.
                if sp_seen.is_some_and(|old| *sp > old) {
                    out.write(Sfx::new(Snd::Kudos).after(0.15));
                }
                *sp_seen = Some(*sp);
            }
            Event::Interacted { unit, prop, .. } if *unit == player => {
                if let Some(snd) = sim.content().prop(prop).and_then(|d| prop_sfx(d.kind)) {
                    out.write(Sfx::new(snd));
                }
            }
            Event::MinigameEnded { result } if result.unit == player => {
                match result.medal.filter(|_| !result.quit) {
                    Some(Medal::Gold) => {
                        out.write(Sfx::new(Snd::MedalGold));
                        out.write(Sfx::new(Snd::VoiceFlawless).after(0.9));
                    }
                    Some(Medal::Silver) => {
                        out.write(Sfx::new(Snd::MedalSilver));
                    }
                    Some(Medal::Bronze) => {
                        out.write(Sfx::new(Snd::MedalBronze));
                    }
                    None => {}
                }
            }
            // Staged after the fanfare so the two do not mask each other.
            Event::SpellLearned { unit, .. } if *unit == player => {
                out.write(Sfx::new(Snd::SpellLearned).after(1.2));
            }
            Event::ZoneCleared { zone } => {
                out.write(Sfx::new(Snd::FloorCleared));
                // Only tower floors have an elevator that opens on the clear.
                if zone.starts_with("tower/") {
                    out.write(Sfx::new(Snd::ElevatorDing).after(0.9));
                    out.write(Sfx::new(Snd::ElevatorDoor).after(1.1));
                }
            }
            Event::QuestCompleted { .. } => {
                out.write(Sfx::new(Snd::QuestDone));
            }
            Event::ZoneLoaded { .. } => {
                // The first load happens at startup, long before any gesture.
                let seen_before = std::mem::replace(&mut *first_floor, true);
                if seen_before {
                    out.write(Sfx::new(Snd::ElevatorDoor).after(0.5));
                }
            }
            _ => {}
        }
    }
    // Hits are heard when their projectile lands, like the floating numbers.
    for PresentEvent(ev) in present.read() {
        match ev {
            Event::Damage {
                source,
                target,
                spell,
                crit,
                amount,
                absorbed,
                ..
            } if *amount > 0 => {
                if *target == player {
                    out.write(Sfx::new(Snd::HitPlayer).gain(near(*source)));
                    if *absorbed > 0 {
                        out.write(Sfx::new(Snd::Block).gain(near(*source)));
                    }
                } else if *source == player {
                    let hit = if is_periodic(sim.content(), *spell) {
                        Sfx::new(Snd::Tick).gain(0.7)
                    } else if spell.is_some() {
                        Sfx::new(Snd::HitSpell)
                    } else {
                        Sfx::new(Snd::HitLight)
                    };
                    out.write(hit.gain(near(*target)));
                    if *crit {
                        out.write(Sfx::new(Snd::HitCrit).gain(near(*target)));
                    }
                }
            }
            Event::Miss { source, target, .. } if *source == player || *target == player => {
                out.write(Sfx::new(Snd::Miss).gain(near(*source).max(near(*target))));
            }
            _ => {}
        }
    }
}

fn play_sfx(
    time: Res<Time<Real>>,
    mut msgs: MessageReader<Sfx>,
    mut bank: ResMut<Bank>,
    settings: Res<AudioSettings>,
    unlocked: Res<Unlocked>,
    voices: Query<(), With<SfxVoice>>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    // Before the first gesture nothing may reach the (suspended) browser context, or it
    // would all play at once when the context resumes.
    if !unlocked.0 {
        msgs.clear();
        bank.pending.clear();
        return;
    }
    bank.pending.extend(msgs.read().copied());
    let mut ready = Vec::new();
    bank.pending.retain_mut(|s| {
        s.delay -= dt;
        if s.delay <= 0.0 {
            ready.push(*s);
            false
        } else {
            true
        }
    });
    let mut live = voices.iter().count();
    for s in ready {
        let Some(d) = TABLE.iter().find(|d| d.snd == s.snd) else {
            continue;
        };
        let gain = d.vol * s.gain * settings.effective(Group::Sfx);
        if gain < MIN_GAIN
            || live >= MAX_VOICES
            || bank
                .last_played
                .get(&s.snd)
                .is_some_and(|t| now - t < d.cooldown)
        {
            continue;
        }
        let pick = (bank.rand() * d.files.len() as f32) as usize;
        let Some(clip) = bank
            .clips
            .get(&s.snd)
            .and_then(|c| c.get(pick.min(c.len().saturating_sub(1))))
            .cloned()
        else {
            continue;
        };
        let speed = if d.jitter {
            1.0 + (bank.rand() * 2.0 - 1.0) * PITCH_JITTER
        } else {
            1.0
        };
        bank.last_played.insert(s.snd, now);
        live += 1;
        commands.spawn((
            SfxVoice,
            AudioPlayer::new(clip),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(gain))
                .with_speed(speed),
        ));
    }
}

/// Exploration loop on floors, boss loop while a boss is fighting, nothing on the victory screen.
fn mood(flow: &GameFlow, sim: &dyn SimRead) -> Mood {
    if flow.victory {
        Mood::Silent
    } else if sim.units().any(|u| u.boss && u.alive && u.in_combat) {
        Mood::Boss
    } else {
        Mood::Explore
    }
}

fn wants(track: Track, mood: Mood) -> bool {
    match track {
        Track::Explore => mood == Mood::Explore,
        Track::Boss => mood == Mood::Boss,
        Track::Ambient => mood != Mood::Silent,
    }
}

fn approach(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

fn drive_music(
    time: Res<Time<Real>>,
    unlocked: Res<Unlocked>,
    settings: Res<AudioSettings>,
    flow: Res<GameFlow>,
    day: Res<crate::live::DayNight>,
    assets: Res<AssetServer>,
    sim: SimView,
    bank: Option<Res<Bank>>,
    mut voices: Query<(Entity, &mut MusicVoice, Option<&mut AudioSink>)>,
    mut commands: Commands,
) {
    if bank.is_none() || !unlocked.0 {
        return;
    }
    let mood = mood(&flow, &*sim);
    let step = time.delta_secs() / CROSSFADE_SECS;
    let night = flow.outdoor && day.darkness > 0.6;
    for t in TRACKS {
        let want = wants(t.track, mood);
        let file = music_file(t.track, &flow.zone, night);
        let mut have = false;
        for (entity, mut voice, sink) in voices.iter_mut().filter(|(_, v, _)| v.track == t.track) {
            let current = voice.file == file;
            have |= current;
            let on = want && current;
            voice.fade = approach(voice.fade, if on { 1.0 } else { 0.0 }, step);
            // The boss loop restarts from its beginning on every fight; a zone's old loop goes
            // once it has crossfaded out.
            if voice.fade == 0.0 && !on && (t.track == Track::Boss || !current) {
                commands.entity(entity).despawn();
                continue;
            }
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(
                    voice.fade * t.level * settings.effective(t.group),
                ));
                // A silent loop keeps its position instead of decoding for nothing.
                match (voice.fade == 0.0 && !on, sink.is_paused()) {
                    (true, false) => sink.pause(),
                    (false, true) => sink.play(),
                    _ => {}
                }
            }
        }
        if want && !have {
            commands.spawn((
                MusicVoice {
                    track: t.track,
                    file,
                    fade: 0.0,
                },
                AudioPlayer::new(assets.load(file)),
                // Silent until the fade-in above; the sink appears one frame later.
                PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
            ));
        }
    }
}

#[derive(Component)]
struct AudioPanel;

#[derive(Component)]
struct VolButton(Group, f32);

#[derive(Component)]
struct MuteButton;

#[derive(Component)]
enum PanelText {
    Vol(Group),
    Mute,
}

fn keycap() -> impl Bundle {
    (
        Button,
        Node {
            padding: UiRect::axes(px(12), px(3)),
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BorderColor::all(KEY_RIM),
        vgrad(KEY_TOP_HI, KEY_TOP),
    )
}

/// Volume controls under the pause message (shown only while paused).
fn spawn_panel(mut commands: Commands) {
    commands
        .spawn((
            AudioPanel,
            Node {
                position_type: PositionType::Absolute,
                top: percent(56),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(51),
        ))
        .with_children(|c| {
            for (name, group) in [
                ("Master", Group::Master),
                ("Music", Group::Music),
                ("Effects", Group::Sfx),
            ] {
                c.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((
                        label(name, 18.0),
                        Node {
                            width: px(90),
                            ..default()
                        },
                    ));
                    r.spawn((keycap(), VolButton(group, -VOLUME_STEP)))
                        .with_child(label("-", 18.0));
                    r.spawn((
                        PanelText::Vol(group),
                        label("", 18.0),
                        Node {
                            width: px(52),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ));
                    r.spawn((keycap(), VolButton(group, VOLUME_STEP)))
                        .with_child(label("+", 18.0));
                });
            }
            c.spawn((keycap(), MuteButton))
                .with_child((PanelText::Mute, label("", 18.0)));
        });
}

fn audio_panel(
    paused: Res<Paused>,
    mut settings: ResMut<AudioSettings>,
    mut panel: Single<&mut Node, With<AudioPanel>>,
    steps: Query<(&Interaction, &VolButton), Changed<Interaction>>,
    mute: Query<&Interaction, (With<MuteButton>, Changed<Interaction>)>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut out: MessageWriter<Sfx>,
) {
    panel.display = if paused.0 {
        Display::Flex
    } else {
        Display::None
    };
    if !paused.0 {
        return;
    }
    for (i, VolButton(group, delta)) in &steps {
        if *i == Interaction::Pressed {
            settings.step(*group, *delta);
            out.write(Sfx::new(Snd::Key));
        }
    }
    if mute.iter().any(|i| *i == Interaction::Pressed) {
        settings.muted = !settings.muted;
        remember_mute(settings.muted);
    }
    for (kind, mut text) in &mut texts {
        match kind {
            PanelText::Vol(g) => {
                set_text(&mut text, &format!("{:.0}%", settings.slider(*g) * 100.0))
            }
            PanelText::Mute => set_text(
                &mut text,
                if settings.muted {
                    "[M] Muted: click to unmute"
                } else {
                    "[M] Mute"
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    fn asset(path: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(path)
    }

    #[test]
    fn every_clip_exists_and_ids_are_unique() {
        let mut seen = HashSet::new();
        for d in TABLE {
            assert!(seen.insert(d.snd), "duplicate {:?}", d.snd);
            assert!(
                !d.files.is_empty() && d.vol > 0.0 && d.vol <= 1.0,
                "{:?}",
                d.snd
            );
            for f in d.files {
                assert!(asset(f).exists(), "{f}");
            }
        }
        let Ok(content) = combat_core::Content::from_ron(include_str!("../../../content/game.ron"))
        else {
            panic!("content");
        };
        for zone in &content.zones {
            for t in TRACKS {
                for night in [false, true] {
                    let f = music_file(t.track, &zone.id, night);
                    assert!(asset(f).exists(), "{} -> {f}", zone.id);
                }
            }
        }
    }

    #[test]
    fn spell_table_matches_content() {
        let Ok(sim) = crate::combat::new_sim() else {
            panic!("embedded content invalid");
        };
        for s in SPELL_SFX {
            let def = sim.content().spell(SpellId(s.id));
            assert_eq!(def.map(|d| d.name.as_str()), Some(s.name), "spell {}", s.id);
            assert!(TABLE.iter().any(|d| d.snd == s.snd));
        }
    }

    #[test]
    fn falloff_is_one_at_the_player_and_zero_beyond_earshot() {
        let t = |d: f32| (1.0 - d / HEARD_M).clamp(0.0, 1.0).powi(2);
        assert_eq!(t(0.0), 1.0);
        assert_eq!(t(HEARD_M * 2.0), 0.0);
        assert!(t(10.0) > t(20.0));
    }

    #[test]
    fn crossfade_reaches_the_target_without_overshoot() {
        let mut f = 0.0;
        for _ in 0..100 {
            f = approach(f, 1.0, 0.1);
        }
        assert_eq!(f, 1.0);
        assert_eq!(approach(0.05, 0.0, 0.1), 0.0);
    }

    #[test]
    fn volume_steps_stay_on_the_grid() {
        let mut s = AudioSettings::default();
        for _ in 0..20 {
            s.step(Group::Music, VOLUME_STEP);
        }
        assert_eq!(s.music, 1.0);
        for _ in 0..30 {
            s.step(Group::Music, -VOLUME_STEP);
        }
        assert_eq!(s.music, 0.0);
    }
}
