//! Seeing and changing the queue.

use crate::data::{Context, Error, LoopMode};
use crate::discord_ui;
use crate::player::{self, current_call, require_same_channel, Shared};
use crate::util;
use rand::seq::SliceRandom;

/// Show what's playing and what's next
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("q", "list", "setlist"))]
pub async fn queue(
    ctx: Context<'_>,
    #[description = "Page"]
    #[min = 1]
    page: Option<usize>,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let shared = Shared::from_ctx(ctx).await;
    let (embed, rows) = discord_ui::setlist_page(&shared, guild, page.unwrap_or(1)).await;
    ctx.send(poise::CreateReply::default().embed(embed).components(rows)).await?;
    Ok(())
}

/// Show the current song with control buttons
#[poise::command(slash_command, prefix_command, category = "Queue", guild_only, aliases("np", "now"))]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let shared = Shared::from_ctx(ctx).await;
    let (embed, rows) = discord_ui::now_playing_card(&shared, guild).await.ok_or("Nothing is playing.")?;
    ctx.send(poise::CreateReply::default().embed(embed).components(rows)).await?;
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
