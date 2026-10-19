//! Playing and controlling music.

use crate::data::{Context, Error};
use crate::player::{self, current_call, require_same_channel, Shared};
use crate::{source, util};
use std::time::Duration;

/// Play a song or playlist (link or search)
#[poise::command(slash_command, prefix_command, category = "Music", guild_only, aliases("p"))]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Song name (start with video: for the music video), or a link"]
    #[rest]
    query: String,
) -> Result<(), Error> {
    add_songs(ctx, query, false).await
}

/// Play a song right after the current one
#[poise::command(slash_command, prefix_command, category = "Music", guild_only, aliases("pn", "playtop"))]
pub async fn playnext(
    ctx: Context<'_>,
    #[description = "Song name or link"]
    #[rest]
    query: String,
) -> Result<(), Error> {
    add_songs(ctx, query, true).await
}

async fn add_songs(ctx: Context<'_>, query: String, next: bool) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    // Looking songs up takes a second or two.
    ctx.defer().await?;

    let call = player::join(ctx).await?;
    player::remember_text_channel(ctx.data(), guild, ctx.channel_id());
    let songs = source::resolve(&query, ctx.author().id).await?;
    let shared = Shared::from_ctx(ctx).await;

    let was_empty = call.lock().await.queue().is_empty();
    let count = songs.len();
    let first = songs[0].clone();
    for song in songs {
        player::enqueue(&shared, guild, &call, song).await;
    }

    // playnext: move the new songs from the end to right after the current one.
    if next && !was_empty {
        call.lock().await.queue().modify_queue(|queue| {
            let added: Vec<_> = (0..count).filter_map(|_| queue.pop_back()).collect();
            for song in added {
                queue.insert(1, song);
            }
        });
    }

    let reply = if count > 1 {
        util::embed().description(format!("📃 A-added **{count}** songs from the playlist... that's a lot of songs..."))
    } else if was_empty {
        // "Now playing" is posted when it actually starts.
        util::embed().description(format!("🎸 O-okay... getting **{}** ready...", first.title))
    } else {
        let position = if next { "next".to_string() } else { format!("#{}", call.lock().await.queue().len() - 1) };
        util::song_embed(&format!("🎸 Added to the queue · {position}"), &first)
    };
    ctx.send(poise::CreateReply::default().embed(reply)).await?;
    Ok(())
}

/// Pause the music
#[poise::command(slash_command, prefix_command, category = "Music", guild_only)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    current_call(ctx).await?.lock().await.queue().pause()?;
    util::say(ctx, "📦 Paused. I'll hide in my box until you resume...").await
}

/// Keep playing after a pause
#[poise::command(slash_command, prefix_command, category = "Music", guild_only, aliases("unpause"))]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    current_call(ctx).await?.lock().await.queue().resume()?;
    util::say(ctx, "▶️ C-coming out of the box... playing again").await
}

/// Skip the current song
#[poise::command(slash_command, prefix_command, category = "Music", guild_only, aliases("s", "next"))]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let call = call.lock().await;
    let current = call.queue().current().ok_or("Nothing is playing.")?;
    let title = player::info(&current).title.clone();
    call.queue().skip()?;
    util::say(ctx, format!("⏭️ Skipped **{title}**... s-sorry, was it that bad?")).await
}

/// Jump to a song in the queue, skipping everything before it
#[poise::command(slash_command, prefix_command, category = "Music", guild_only, aliases("jump"))]
pub async fn skipto(
    ctx: Context<'_>,
    #[description = "Position in the queue"]
    #[min = 1]
    position: usize,
) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let call = call.lock().await;
    if position == 0 || position >= call.queue().len() {
        return Err("There's no song at that position. Check `queue`.".into());
    }
    // Drop the songs between the current one and the target, then skip the current one.
    call.queue().modify_queue(|queue| {
        for skipped in queue.drain(1..position) {
            let _ = skipped.stop();
        }
    });
    call.queue().skip()?;
    util::say(ctx, format!("⏭️ Jumped to song #{position}")).await
}

/// Stop the music and clear the queue
#[poise::command(slash_command, prefix_command, category = "Music", guild_only)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    current_call(ctx).await?.lock().await.queue().stop();
    util::say(ctx, "⏹️ Stopped and cleared the queue. Th-the concert's over...").await
}

/// Jump to a time in the song, e.g. `seek 1:30`
#[poise::command(slash_command, prefix_command, category = "Music", guild_only)]
pub async fn seek(
    ctx: Context<'_>,
    #[description = "Time, e.g. 1:30 or 90"] time: String,
) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let position = util::parse_time(&time).ok_or("Couldn't read that time. Try `1:30` or `90`.")?;
    let call = current_call(ctx).await?;
    let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
    if let Some(length) = player::info(&current).duration {
        if position >= length {
            return Err(format!("This song is only {} long.", util::format_time(length)).into());
        }
    }
    current
        .seek_async(position)
        .await
        .map_err(|_| "Couldn't seek in this song.")?;
    util::say(ctx, format!("⏩ Jumped to **{}**", util::format_time(position))).await
}

/// Go back to the start of the song
#[poise::command(slash_command, prefix_command, category = "Music", guild_only)]
pub async fn replay(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let call = current_call(ctx).await?;
    let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
    current.seek_async(Duration::ZERO).await.map_err(|_| "Couldn't restart this song.")?;
    util::say(ctx, "⏮️ Back to the start").await
}
