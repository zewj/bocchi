//! Joining, leaving, volume and help.

use crate::data::{Context, Error};
use crate::player::{self, current_call, require_same_channel};
use crate::util;

/// Join your voice channel
#[poise::command(slash_command, prefix_command, category = "General", guild_only, aliases("summon"))]
pub async fn join(ctx: Context<'_>) -> Result<(), Error> {
    player::join(ctx).await?;
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    player::remember_text_channel(ctx.data(), guild, ctx.channel_id());
    let channel = player::author_voice_channel(ctx).ok_or("Join a voice channel first.")?;
    util::say(ctx, format!("👋 Joined <#{channel}>")).await
}

/// Leave the voice channel and clear the queue
#[poise::command(slash_command, prefix_command, category = "General", guild_only, aliases("dc", "disconnect", "bye"))]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    require_same_channel(ctx)?;
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let manager = player::manager(ctx).await;
    if let Some(call) = manager.get(guild) {
        call.lock().await.queue().stop();
    }
    manager.remove(guild).await?;
    util::say(ctx, "👋 Left the voice channel").await
}

/// Set the volume (0-200%)
#[poise::command(slash_command, prefix_command, category = "General", guild_only, aliases("vol", "v"))]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume in percent (0-200)"]
    #[min = 0]
    #[max = 200]
    percent: Option<u32>,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("This only works in a server.")?;
    let Some(percent) = percent else {
        let current = ctx.data().with_guild(guild, |s| s.volume);
        return util::say(ctx, format!("🔊 Volume is **{:.0}%**", current * 100.0)).await;
    };
    require_same_channel(ctx)?;
    if percent > 200 {
        return Err("Volume goes up to 200%.".into());
    }
    let level = percent as f32 / 100.0;
    ctx.data().with_guild(guild, |s| s.volume = level);

    // Apply it to every queued song, not just the current one.
    for song in current_call(ctx).await?.lock().await.queue().current_queue() {
        let _ = song.set_volume(level);
    }
    let icon = match percent {
        0 => "🔇",
        1..=50 => "🔈",
        51..=100 => "🔉",
        _ => "🔊",
    };
    util::say(ctx, format!("{icon} Volume set to **{percent}%**")).await
}

/// Show all commands
#[poise::command(slash_command, prefix_command, category = "General", aliases("h", "commands"))]
pub async fn help(
    ctx: Context<'_>,
    #[description = "A specific command"]
    #[rest]
    command: Option<String>,
) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        ephemeral: true,
        extra_text_at_bottom: "Bocchi",
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

/// Check the bot's latency
#[poise::command(slash_command, prefix_command, category = "General")]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let gateway = ctx.ping().await.as_millis();
    util::say(ctx, format!("🏓 Pong! Gateway **{gateway} ms**")).await
}
