//! Optional online co-op. The game starts and plays fully offline; nothing in here touches the
//! network until the player opens `~/coop` and presses Create or Join.
//!
//! - `transport`: protocol-agnostic binary websocket (`NetClient`, `NetEvent`).
//! - `address`: server address normalisation and text-field filtering.
//! - `lobby`: the Esc-menu entry and the `~/coop` panel.
//! - `session`: handshake, keep-alive, `GameMode` switches, the online tick.
//! - `world`: `NetWorld`, the server's room as seen by this client (implements `SimRead`).
//! - `party`: party frames; `emotes`: the emote menu.
//!
//! Debug: `WOW_COOP=1` starts paused with the `~/coop` panel open (for screenshots);
//! `WOW_ONLINE=<ws url>` connects and creates a room at startup, `WOW_JOIN=<code>` joins one,
//! `WOW_PUBLIC=<title>` makes the created room public, `WOW_BROWSE=1` opens the room browser.

pub mod address;
mod emotes;
#[cfg(test)]
mod harness;
mod lobby;
mod party;
mod session;
#[cfg(target_arch = "wasm32")]
mod soft_keyboard;
mod store;
pub mod transport;
mod world;

use bevy::prelude::*;

// Touch UIs drive the emote menu through these.
#[allow(unused_imports)]
pub use emotes::{EmoteRequest, ToggleEmoteMenu};
pub use lobby::Lobby;
pub use session::{GameMode, LeaveRequest, MoveInput, online_step};
pub use store::{load_class, load_hud, load_keys, save_class, save_hud, save_keys};
pub use transport::{NetClient, NetEvent};
pub use world::NetWorld;

/// Another player's character in an online room.
#[derive(Component)]
pub struct RemotePlayer;

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetClient>()
            .init_resource::<GameMode>()
            .init_resource::<session::Session>()
            .add_message::<NetEvent>()
            .add_message::<LeaveRequest>()
            .add_systems(Startup, session::auto_connect)
            // PreUpdate, so a mode switch's despawns land before any Update system queues
            // commands on those entities (models finishing loading, for one).
            .add_systems(
                PreUpdate,
                (transport::pump_net, session::run_session).chain(),
            )
            .add_systems(Last, transport::close_on_exit)
            .add_plugins((lobby::LobbyPlugin, party::PartyPlugin, emotes::EmotePlugin));
    }
}
