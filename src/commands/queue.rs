//! Seeing and changing the queue.

use crate::data::{Context, Error, LoopMode};
use crate::player::{self, current_call, require_same_channel};
use crate::util;
use poise::ChoiceParameter;
use rand::seq::SliceRandom;
use std::time::Duration;

const PAGE_SIZE: usize = 10;

/// Show what's playing and what's next
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("q", "list"))]
pub async fn queue(
    ctx: Context<'_>,
    #[description = "Page"]
    #[min = 1]
    page: Option<usize>,
) -> Result<(), Error> {
    let call = current_call(ctx).await?;
    let songs = call.lock().await.queue().current_queue();
    let Some((now, upcoming)) = songs.split_first() else {
        return util::say(ctx, "The queue is empty. Add something with `play`.").await;
    };

    let pages = upcoming.len().div_ceil(PAGE_SIZE).max(1);
    let page = page.unwrap_or(1).clamp(1, pages);
    let now_info = player::info(now);

    let lines: Vec<String> = upcoming
        .iter()
        .enumerate()
        .skip((page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|(i, handle)| {
            let song = player::info(handle);
            format!("`{}.` [{}]({}) · {}", i + 1, song.title, song.url, util::format_length(song.duration))
        })
        .collect();

    let total: Duration = songs.iter().filter_map(|h| player::info(h).duration).sum();
    let loop_mode = ctx.data().with_guild(ctx.guild_id().unwrap(), |s| s.loop_mode);
    let up_next = if lines.is_empty() { "Nothing. Add more with `play`.".to_string() } else { lines.join("\n") };

    let embed = util::embed()
        .title("🎶 Queue")
        .description(format!("**Now playing:** [{}]({})\n\n**Up next:**\n{up_next}", now_info.title, now_info.url))
        .footer(util::footer(format!(
            "Page {page}/{pages} · {} songs · {} total · loop: {}",
            songs.len(),
            util::format_time(total),
            loop_mode.name()
        )));
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Show the current song and how far along it is
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("np", "now"))]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let call = current_call(ctx).await?;
    let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
    let song = player::info(&current);
    let state = current.get_info().await?;

    let progress = format!(
        "{} `{} / {}`",
        util::progress_bar(state.position, song.duration),
        util::format_time(state.position),
        util::format_length(song.duration)
    );
    let embed = util::song_embed("🎶 Now playing", &song)
        .description(progress)
        .field("Volume", format!("{:.0}%", state.volume * 100.0), true);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Shuffle the songs that are up next
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("mix"))]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let call = call.lock().await;
    if call.queue().len() < 3 {
        return Err("Need at least 2 songs up next to shuffle.".into());
    }
    call.queue().modify_queue(|queue| {
        // Keep the current song where it is.
        let mut upcoming: Vec<_> = queue.drain(1..).collect();
        upcoming.shuffle(&mut rand::rng());
        queue.extend(upcoming);
    });
    util::say(ctx, "🔀 Shuffled the queue").await
}

/// Remove a song from the queue
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("rm"))]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Position in the queue"]
    #[min = 1]
    position: usize,
) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let removed = call
        .lock()
        .await
        .queue()
        .modify_queue(|queue| if position == 0 { None } else { queue.remove(position) })
        .ok_or("There's no song at that position. Check `queue`.")?;
    let title = player::info(&removed).title.clone();
    let _ = removed.stop();
    util::say(ctx, format!("🗑️ Removed **{title}**")).await
}

/// Remove every song that's up next (keeps the current one)
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only)]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let removed = call.lock().await.queue().modify_queue(|queue| {
        let upcoming: Vec<_> = queue.drain(1..).collect();
        for song in &upcoming {
            let _ = song.stop();
        }
        upcoming.len()
    });
    util::say(ctx, format!("🧹 Removed **{removed}** songs from the queue")).await
}

/// Repeat the song, the whole queue, or nothing
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("repeat"))]
pub async fn r#loop(
    ctx: Context<'_>,
    #[description = "off, song or queue"] mode: LoopMode,
) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    ctx.data().with_guild(guild, |s| s.loop_mode = mode);

    // Song-loop is handled by songbird on the current track itself.
    if let Some(current) = current_call(ctx).await?.lock().await.queue().current() {
        let _ = if mode == LoopMode::Song { current.enable_loop() } else { current.disable_loop() };
    }
    let text = match mode {
        LoopMode::Off => "➡️ Loop off",
        LoopMode::Song => "🔂 Looping this song",
        LoopMode::Queue => "🔁 Looping the queue",
    };
    util::say(ctx, text).await
}
