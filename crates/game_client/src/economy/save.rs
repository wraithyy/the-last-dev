//! Offline persistence: one save per class (level, XP, gear, bag, Kudos, trophies,
//! completed quests, cleared zones) in a native file next to `coop.txt` or in localStorage on
//! the web. Loaded when an offline run starts, written every few seconds when it changed.
//! Online runs are session-only and never touch the save. After the final victory the save
//! keeps the character but resets the world ("New Game+"). Debug launches (`WOW_LEVEL`,
//! `WOW_ZONE`, `WOW_FLOOR`, screenshots, `WOW_NO_SAVE`) neither load nor write.

use crate::combat::{CombatSim, SimReady};
use crate::game::{GameFlow, GameRestarted};
use crate::net::GameMode;
use bevy::prelude::*;
use combat_core::SaveData;

const SAVE_EVERY_SECS: f32 = 5.0;

#[derive(Resource, Default)]
struct SaveState {
    /// The sim player and class the save was loaded into.
    loaded: Option<(u64, String)>,
    last_written: String,
    clock: f32,
    /// A wiped save waits for the restart: the old sim must not write itself back first.
    wiped: bool,
}

/// Esc menu > Start over: forget the class's save; a `RestartRun` sent with it starts again at P3.
#[derive(Message)]
pub struct WipeSave(pub String);

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveState>()
            .add_message::<WipeSave>()
            .add_systems(Update, (wipe, load, write).chain().in_set(SimReady));
    }
}

fn disabled() -> bool {
    [
        "WOW_NO_SAVE",
        "WOW_LEVEL",
        "WOW_ZONE",
        "WOW_FLOOR",
        "WOW_LINEUP",
    ]
    .iter()
    .any(|k| std::env::var_os(k).is_some())
        || crate::debug::shooting()
}

fn wipe(mut wipes: MessageReader<WipeSave>, mut state: ResMut<SaveState>) {
    for WipeSave(class) in wipes.read() {
        platform::delete(class);
        state.wiped = true;
        state.loaded = None;
        state.last_written.clear();
        info!("wiped the {class} save");
    }
}

fn load(
    mut sim: ResMut<CombatSim>,
    mode: Res<GameMode>,
    mut state: ResMut<SaveState>,
    mut restarted: MessageReader<GameRestarted>,
    mut onboarding: ResMut<crate::onboarding::Onboarding>,
) {
    if restarted.read().count() > 0 {
        state.loaded = None;
        state.wiped = false;
    }
    if state.wiped || mode.is_online() || disabled() {
        return;
    }
    let Some(p) = sim.0.player() else { return };
    let Some(class) = sim.0.class_of(p).map(|c| c.key.clone()) else {
        return;
    };
    if state.loaded.as_ref() == Some(&(p, class.clone())) {
        return;
    }
    state.loaded = Some((p, class.clone()));
    let Some(text) = platform::read(&class) else {
        // No save for this class yet: a new hire.
        onboarding.class_loaded(true);
        return;
    };
    match SaveData::decode(&text) {
        Some(save) if save.class == class => {
            onboarding.class_loaded(save.onboarding_pending);
            sim.0.import_save(p, &save);
            state.last_written = text;
            info!(
                "loaded the {class} save: level {}, {} Kudos",
                save.level, save.sp
            );
        }
        _ => {
            warn!("ignoring an unreadable or foreign {class} save");
            onboarding.class_loaded(true);
        }
    }
}

fn write(
    time: Res<Time>,
    sim: Res<CombatSim>,
    mode: Res<GameMode>,
    flow: Res<GameFlow>,
    onboarding: Res<crate::onboarding::Onboarding>,
    mut state: ResMut<SaveState>,
) {
    if mode.is_online() || disabled() || state.loaded.is_none() {
        return;
    }
    state.clock += time.delta_secs();
    if state.clock < SAVE_EVERY_SECS {
        return;
    }
    state.clock = 0.0;
    let Some(mut save) = sim.0.player().and_then(|p| sim.0.export_save(p)) else {
        return;
    };
    save.onboarding_pending = onboarding.pending;
    if flow.victory {
        save.cleared.clear();
        save.bosses.clear();
        save.quests_done.clear();
    }
    let text = save.encode();
    if text != state.last_written {
        platform::write(&save.class, &text);
        state.last_written = text;
    }
}

/// Delivered sprints of an online run go to the local leaderboard of the class too, with the
/// new recommended level (the rest of an online session stays out of the save). Needs an
/// existing save of that class.
pub fn record_online_sprint(
    class: &str,
    record: Option<combat_core::SprintRecord>,
    key: (String, u8),
) {
    if disabled() {
        return;
    }
    let Some(mut save) = platform::read(class).and_then(|t| SaveData::decode(&t)) else {
        return;
    };
    save.sprint_records.extend(record);
    save.sprint_keys.retain(|(b, _)| *b != key.0);
    save.sprint_keys.push(key);
    platform::write(class, &save.encode());
}

/// Online Daily Standup progress goes into the local save of the class too (the room itself is
/// session-only), so an offline session the same day picks it up. Needs an existing save.
pub fn record_online_tasks(class: &str, board: &combat_core::BoardView) {
    if disabled() {
        return;
    }
    let Some(mut save) = platform::read(class).and_then(|t| SaveData::decode(&t)) else {
        return;
    };
    save.tasks_day = board.day;
    save.tasks_week = board.week;
    save.tasks = board
        .tasks
        .iter()
        .filter(|t| t.state != combat_core::TaskState::Open)
        .map(|t| combat_core::SavedTask {
            key: t.key.clone(),
            weekly: t.weekly,
            state: t.state,
            level: t.level,
            counts: t.goals.iter().map(|g| g.count).collect(),
        })
        .collect();
    let text = save.encode();
    if platform::read(class).as_deref() != Some(text.as_str()) {
        platform::write(class, &text);
    }
}

/// The local leaderboard of a class's save (empty without one).
pub fn local_sprint_records(class: &str) -> Vec<combat_core::SprintRecord> {
    if disabled() {
        return Vec::new();
    }
    platform::read(class)
        .and_then(|t| SaveData::decode(&t))
        .map(|s| s.sprint_records)
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::path::PathBuf;

    fn path(class: &str) -> Option<PathBuf> {
        let env = |k| std::env::var_os(k).map(PathBuf::from);
        let base = if cfg!(target_os = "macos") {
            env("HOME")?.join("Library/Application Support")
        } else if cfg!(windows) {
            env("APPDATA")?
        } else {
            env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
        };
        // Class keys are content ids; anything else never reaches the file system.
        let safe = class.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        safe.then(|| base.join("the-last-dev").join(format!("save_{class}.txt")))
    }

    pub fn read(class: &str) -> Option<String> {
        std::fs::read_to_string(path(class)?).ok()
    }

    pub fn write(class: &str, text: &str) {
        let Some(p) = path(class) else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Write then rename, so a crash mid-write never leaves half a save.
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, p);
        }
    }

    pub fn delete(class: &str) {
        if let Some(p) = path(class) {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read(class: &str) -> Option<String> {
        storage()?.get_item(&format!("tld_save_{class}")).ok()?
    }

    pub fn write(class: &str, text: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(&format!("tld_save_{class}"), text);
        }
    }

    pub fn delete(class: &str) {
        if let Some(s) = storage() {
            let _ = s.remove_item(&format!("tld_save_{class}"));
        }
    }
}
