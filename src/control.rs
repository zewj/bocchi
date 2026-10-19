//! What the control panel can see and do: a snapshot of every player, and the button actions.

use crate::bot::Bot;
use crate::data::{Error, LoopMode, TrackInfo};
use crate::{player, source};
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use std::collections::HashMap;
use rand::seq::SliceRandom;
use songbird::tracks::PlayMode;
use std::time::Duration;

/// One server where the bot is in voice.
#[derive(Clone)]
pub struct PlayerView {
    pub guild: GuildId,
    pub server: String,
    pub channel: String,
    /// The current song and how far into it we are.
    pub now: Option<(TrackInfo, Duration)>,
    pub paused: bool,
    pub upcoming: Vec<TrackInfo>,
    pub volume: f32,
    pub loop_mode: LoopMode,
    /// Display names for everyone who requested a song here.
    pub names: HashMap<UserId, String>,
}

/// Reads every active player. Called a couple of times a second by the panel.
pub async fn snapshot(bot: &Bot) -> Vec<PlayerView> {
    let calls: Vec<_> = bot.songbird.iter().map(|(g, call)| (GuildId::new(g.0.get()), call)).collect();
    let mut views = Vec::new();

    for (guild, call) in calls {
        let call = call.lock().await;
        let channel_id = call.current_channel().map(|c| ChannelId::new(c.0.get()));
        let handles = call.queue().current_queue();
        drop(call);

        let requesters: Vec<UserId> = handles.iter().map(|h| player::info(h).requester).collect();
        let (server, channel, names) = bot
            .cache
            .guild(guild)
            .map(|g| {
                let channel = channel_id
                    .and_then(|c| g.channels.get(&c))
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "?".into());
                let names = requesters
                    .iter()
                    .filter_map(|u| g.members.get(u).map(|m| (*u, m.display_name().to_string())))
                    .collect();
                (g.name.clone(), channel, names)
            })
            .unwrap_or_else(|| (guild.to_string(), "?".into(), HashMap::new()));

        let mut now = None;
        let mut paused = false;
        if let Some(current) = handles.first() {
            if let Ok(state) = current.get_info().await {
                paused = state.playing == PlayMode::Pause;
                now = Some(((*player::info(current)).clone(), state.position));
            }
        }
        let upcoming = handles.iter().skip(1).map(|h| (*player::info(h)).clone()).collect();
        let (volume, loop_mode) = {
            let mut states = bot.guilds.lock().unwrap();
            let state = states.entry(guild).or_default();
            (state.volume, state.loop_mode)
        };

        views.push(PlayerView { guild, server, channel, now, paused, upcoming, volume, loop_mode, names });
    }
    views.sort_by(|a, b| a.server.cmp(&b.server));
    views
}

/// A button press in the panel.
pub enum Action {
    Pause,
    Resume,
    Skip,
    Stop,
    Replay,
    Shuffle,
    Leave,
    Volume(f32),
    Loop(LoopMode),
    Remove(usize),
    /// Move a song from one queue position to another.
    Move(usize, usize),
    Seek(Duration),
    Add(String),
}

/// Does an action and returns a short message for the log.
pub async fn run(bot: &Bot, guild: GuildId, action: Action) -> Result<String, Error> {
    let call = bot.songbird.get(guild).ok_or("The bot isn't in voice there anymore.")?;

    let message = match action {
        Action::Pause => {
            call.lock().await.queue().pause()?;
            "Paused".to_string()
        }
        Action::Resume => {
            call.lock().await.queue().resume()?;
            "Resumed".to_string()
        }
        Action::Skip => {
            call.lock().await.queue().skip()?;
            "Skipped".to_string()
        }
        Action::Stop => {
            call.lock().await.queue().stop();
            "Stopped and cleared the queue".to_string()
        }
        Action::Replay => {
            let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
            current.seek_async(Duration::ZERO).await.map_err(|_| "Couldn't restart this song.")?;
            "Back to the start".to_string()
        }
        Action::Shuffle => {
            call.lock().await.queue().modify_queue(|queue| {
                if queue.len() > 2 {
                    let mut upcoming: Vec<_> = queue.drain(1..).collect();
                    upcoming.shuffle(&mut rand::rng());
                    queue.extend(upcoming);
                }
            });
            "Shuffled the queue".to_string()
        }
        Action::Leave => {
            call.lock().await.queue().stop();
            bot.songbird.remove(guild).await?;
            "Left the voice channel".to_string()
        }
        Action::Volume(level) => {
            bot.guilds.lock().unwrap().entry(guild).or_default().volume = level;
            for song in call.lock().await.queue().current_queue() {
                let _ = song.set_volume(level);
            }
            // Not logged: dragging the slider would flood the log.
            String::new()
        }
        Action::Loop(mode) => {
            bot.guilds.lock().unwrap().entry(guild).or_default().loop_mode = mode;
            if let Some(current) = call.lock().await.queue().current() {
                let _ = if mode == LoopMode::Song { current.enable_loop() } else { current.disable_loop() };
            }
            "Loop mode changed".to_string()
        }
        Action::Remove(position) => {
            let removed = call
                .lock()
                .await
                .queue()
                .modify_queue(|queue| if position == 0 { None } else { queue.remove(position) })
                .ok_or("That song isn't in the queue anymore.")?;
            let title = player::info(&removed).title.clone();
            let _ = removed.stop();
            format!("Removed {title}")
        }
        Action::Move(from, to) => {
            let moved = call.lock().await.queue().modify_queue(|queue| {
                // Position 0 is the song playing now; it stays put.
                if from == 0 || to == 0 || from >= queue.len() || to >= queue.len() {
                    return false;
                }
                if let Some(song) = queue.remove(from) {
                    queue.insert(to, song);
                }
                true
            });
            if !moved {
                return Err("That song can't move there.".into());
            }
            format!("Moved song #{from} to #{to}")
        }
        Action::Seek(position) => {
            let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
            current.seek_async(position).await.map_err(|_| "Couldn't jump in this song.")?;
            format!("Jumped to {}", crate::util::format_time(position))
        }
        Action::Add(query) => {
            // Songs added from the panel count as requested by the bot itself.
            let me = bot.cache.current_user().id;
            let songs = source::resolve(&query, me).await?;
            let count = songs.len();
            let first = songs[0].title.clone();
            let shared = bot.shared();
            for song in songs {
                player::enqueue(&shared, guild, &call, song).await;
            }
            if count > 1 { format!("Added {count} songs") } else { format!("Added {first}") }
        }
    };
    Ok(message)
}
