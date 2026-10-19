//! Discord events: leave voice when everyone else has left.

use crate::data::{Data, Error};
use crate::{player, util};
use poise::serenity_prelude::{self as serenity, CreateMessage, FullEvent, GuildId};
use std::time::Duration;

/// How long to wait in an empty channel before leaving (in case people come back).
const ALONE_GRACE: Duration = Duration::from_secs(60);

pub async fn handle(ctx: &serenity::Context, event: &FullEvent, data: &Data) -> Result<(), Error> {
    if let FullEvent::VoiceStateUpdate { new, .. } = event {
        let Some(guild) = new.guild_id else { return Ok(()) };
        if is_alone(ctx, guild) {
            let ctx = ctx.clone();
            let text_channel = data.with_guild(guild, |s| s.text_channel);
            tokio::spawn(async move {
                tokio::time::sleep(ALONE_GRACE).await;
                if !is_alone(&ctx, guild) {
                    return;
                }
                let Some(manager) = songbird::get(&ctx).await else { return };
                if let Some(call) = manager.get(guild) {
                    call.lock().await.queue().stop();
                }
                let _ = manager.remove(guild).await;
                crate::log!("Left voice in {guild}: everyone left");
                if let Some(channel) = text_channel {
                    let embed = util::embed().description("😰 Everyone left... g-going back to my closet.");
                    let _ = channel.send_message(&ctx.http, CreateMessage::new().embed(embed)).await;
                }
            });
        }
    }
    Ok(())
}

/// True if the bot is in a voice channel with no real people (only bots, or nobody).
fn is_alone(ctx: &serenity::Context, guild: GuildId) -> bool {
    let Some(channel) = player::bot_voice_channel(ctx, guild) else { return false };
    let Some(guild) = ctx.cache.guild(guild) else { return false };
    let people = guild
        .voice_states
        .values()
        .filter(|v| v.channel_id == Some(channel))
        .filter(|v| v.member.as_ref().is_none_or(|m| !m.user.bot))
        .filter(|v| guild.members.get(&v.user_id).is_none_or(|m| !m.user.bot))
        .count();
    people == 0
}
