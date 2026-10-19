//! Shared state and the type aliases every command uses.

use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

pub type Guilds = Arc<Mutex<HashMap<GuildId, GuildState>>>;

pub struct Data {
    /// Per-server player settings. Shared with track events and the control panel, hence the Arc.
    pub guilds: Guilds,
}

impl Data {
    /// Runs `f` on a server's settings, creating defaults the first time.
    pub fn with_guild<T>(&self, guild: GuildId, f: impl FnOnce(&mut GuildState) -> T) -> T {
        f(self.guilds.lock().unwrap().entry(guild).or_default())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, poise::ChoiceParameter)]
pub enum LoopMode {
    #[default]
    #[name = "off"]
    Off,
    /// Repeat the current song
    #[name = "song"]
    Song,
    /// When a song ends, put it back at the end of the queue
    #[name = "queue"]
    Queue,
}

#[derive(Clone)]
pub struct GuildState {
    /// 1.0 = 100%.
    pub volume: f32,
    pub loop_mode: LoopMode,
    /// Where "Now playing" messages go: the channel `play` was last used in.
    pub text_channel: Option<ChannelId>,
}

impl Default for GuildState {
    fn default() -> Self {
        Self { volume: 1.0, loop_mode: LoopMode::Off, text_channel: None }
    }
}

/// What we know about a song. Attached to each track in the queue.
#[derive(Clone)]
pub struct TrackInfo {
    pub title: String,
    pub url: String,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub requester: UserId,
}
