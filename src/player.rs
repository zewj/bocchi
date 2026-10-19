//! The player: joining voice, adding songs to the queue, and reacting when songs start or end.

use crate::data::{Context, Data, Error, GuildState, LoopMode, TrackInfo};
use crate::util;
use async_trait::async_trait;
use poise::serenity_prelude::{self as serenity, ChannelId, CreateMessage, GuildId, Http};
use songbird::events::{Event, EventContext, EventHandler, TrackEvent};
use songbird::tracks::{PlayMode, Track, TrackHandle};
use songbird::{Call, Songbird};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

/// Leave voice after this long with nothing playing.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub type CallLock = Arc<AsyncMutex<Call>>;

pub async fn manager(ctx: Context<'_>) -> Arc<Songbird> {
    songbird::get(ctx.serenity_context()).await.expect("songbird is registered in main.rs")
}

/// The voice channel the command's author is in.
pub fn author_voice_channel(ctx: Context<'_>) -> Option<ChannelId> {
    let guild = ctx.guild()?;
    guild.voice_states.get(&ctx.author().id)?.channel_id
}

/// The voice channel the bot is in, in this server.
pub fn bot_voice_channel(ctx: &serenity::Context, guild: GuildId) -> Option<ChannelId> {
    let me = ctx.cache.current_user().id;
    ctx.cache.guild(guild)?.voice_states.get(&me)?.channel_id
}

/// Joins the author's voice channel (or reuses the current connection).
pub async fn join(ctx: Context<'_>) -> Result<CallLock, Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let target = author_voice_channel(ctx).ok_or("Join a voice channel first.")?;
    let manager = manager(ctx).await;

    if let Some(current) = bot_voice_channel(ctx.serenity_context(), guild) {
        if let Some(call) = manager.get(guild) {
            if current == target {
                return Ok(call);
            }
            let busy = call.lock().await.queue().current().is_some();
            if busy {
                return Err(format!("I'm already playing in <#{current}>.").into());
            }
        }
    }

    let call = manager.join(guild, target).await?;
    // Deafened bots use less bandwidth and it shows people we aren't listening.
    let _ = call.lock().await.deafen(true).await;
    Ok(call)
}

/// The current connection, for commands that need music to be playing.
pub async fn current_call(ctx: Context<'_>) -> Result<CallLock, Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    manager(ctx).await.get(guild).ok_or_else(|| "I'm not in a voice channel.".into())
}

/// Makes sure the author is in the same voice channel as the bot before they control it.
pub fn require_same_channel(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let bot = bot_voice_channel(ctx.serenity_context(), guild).ok_or("I'm not in a voice channel.")?;
    if author_voice_channel(ctx) != Some(bot) {
        return Err(format!("Join <#{bot}> to control the music.").into());
    }
    Ok(())
}

pub fn info(handle: &TrackHandle) -> Arc<TrackInfo> {
    handle.data::<TrackInfo>()
}

/// Everything a track event needs, so it can work without a command context.
#[derive(Clone)]
pub struct Shared {
    pub http: Arc<Http>,
    pub songbird: Arc<Songbird>,
    pub guilds: Arc<Mutex<HashMap<GuildId, GuildState>>>,
}

impl Shared {
    pub async fn from_ctx(ctx: Context<'_>) -> Self {
        Self {
            http: ctx.serenity_context().http.clone(),
            songbird: manager(ctx).await,
            guilds: ctx.data().guilds.clone(),
        }
    }

    fn state<T>(&self, guild: GuildId, f: impl FnOnce(&mut GuildState) -> T) -> T {
        f(self.guilds.lock().unwrap().entry(guild).or_default())
    }
}

/// Adds a song to the end of the queue. It starts right away if nothing is playing.
pub async fn enqueue(shared: &Shared, guild: GuildId, call: &CallLock, song: TrackInfo) -> TrackHandle {
    let input = crate::source::YtDlpStream::input(song.url.clone(), song.duration);
    let track = Track::new_with_data(input, Arc::new(song));
    let volume = shared.state(guild, |s| s.volume);

    let handle = call.lock().await.enqueue(track).await;
    let _ = handle.set_volume(volume);

    let announced = Arc::new(AtomicBool::new(false));
    let _ = handle.add_event(Event::Track(TrackEvent::Play), SongStarted { shared: shared.clone(), guild, announced });
    let _ = handle.add_event(Event::Track(TrackEvent::End), SongEnded { shared: shared.clone(), guild });
    let _ = handle.add_event(Event::Track(TrackEvent::Error), SongFailed { shared: shared.clone(), guild });
    handle
}

pub fn remember_text_channel(data: &Data, guild: GuildId, channel: ChannelId) {
    data.with_guild(guild, |s| s.text_channel = Some(channel));
}

// ============================== Track events ==============================

/// Posts "Now playing" (once per song, not on every resume) and applies song-loop.
struct SongStarted {
    shared: Shared,
    guild: GuildId,
    announced: Arc<AtomicBool>,
}

#[async_trait]
impl EventHandler for SongStarted {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else { return None };
        let (_, handle) = tracks.first()?;
        if self.announced.swap(true, Ordering::SeqCst) {
            return None; // a resume, not a new song
        }
        let (channel, loop_mode) = self.shared.state(self.guild, |s| (s.text_channel, s.loop_mode));
        if loop_mode == LoopMode::Song {
            let _ = handle.enable_loop();
        }
        if let Some(channel) = channel {
            let embed = util::song_embed("🎶 Now playing", &info(handle));
            let _ = channel.send_message(&self.shared.http, CreateMessage::new().embed(embed)).await;
        }
        None
    }
}

/// Re-adds the song for queue-loop, and leaves voice if the queue stays empty.
struct SongEnded {
    shared: Shared,
    guild: GuildId,
}

#[async_trait]
impl EventHandler for SongEnded {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else { return None };
        let (_, handle) = tracks.first()?;
        let Some(call) = self.shared.songbird.get(self.guild) else { return None };

        if self.shared.state(self.guild, |s| s.loop_mode) == LoopMode::Queue {
            let song = (*info(handle)).clone();
            enqueue(&self.shared, self.guild, &call, song).await;
            return None;
        }

        // Nothing left? Wait a bit, then leave if still idle.
        if call.lock().await.queue().is_empty() {
            let shared = self.shared.clone();
            let guild = self.guild;
            tokio::spawn(async move {
                tokio::time::sleep(IDLE_TIMEOUT).await;
                let Some(call) = shared.songbird.get(guild) else { return };
                if call.lock().await.queue().is_empty() {
                    let _ = shared.songbird.remove(guild).await;
                    let channel = shared.state(guild, |s| s.text_channel);
                    if let Some(channel) = channel {
                        let embed = util::embed().description("👋 Left the voice channel because nothing was playing.");
                        let _ = channel.send_message(&shared.http, CreateMessage::new().embed(embed)).await;
                    }
                }
            });
        }
        None
    }
}

/// Tells the channel when a song can't be played (removed video, region lock, ...).
struct SongFailed {
    shared: Shared,
    guild: GuildId,
}

#[async_trait]
impl EventHandler for SongFailed {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else { return None };
        let (state, handle) = tracks.first()?;
        let song = info(handle);
        let reason = match &state.playing {
            PlayMode::Errored(e) => explain(&e.to_string()),
            _ => "unknown error".to_string(),
        };
        eprintln!("[player] couldn't play {} ({}): {reason}", song.title, song.url);

        if let Some(channel) = self.shared.state(self.guild, |s| s.text_channel) {
            let text = format!("⚠️ Couldn't play **{}**, skipping it.
-# {reason}", song.title);
            let embed = util::embed().color(util::FAILURE).description(text);
            let _ = channel.send_message(&self.shared.http, CreateMessage::new().embed(embed)).await;
        }
        None
    }
}

/// Turns a raw playback error into something a person can act on.
fn explain(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("no suitable format reader") || lower.contains("end of stream") {
        "yt-dlp couldn't download it. It may be private, age-restricted or region-locked,          or yt-dlp needs updating (restart the bot, it updates on start)."
            .to_string()
    } else if lower.contains("couldn't start yt-dlp") {
        "yt-dlp isn't installed or isn't on PATH.".to_string()
    } else {
        error.to_string()
    }
}
