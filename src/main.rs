//! Bocchi: a Discord music bot.
//! Audio comes from yt-dlp (YouTube, SoundCloud and more) and is played through songbird.

mod commands;
mod data;
mod events;
mod player;
mod source;
mod util;

use data::{Data, Error};
use poise::serenity_prelude as serenity;
use songbird::SerenityInit;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").map_err(|_| "Set DISCORD_TOKEN in .env")?;
    let prefix = std::env::var("PREFIX").unwrap_or_else(|_| "!".into());
    // Optional: register slash commands in one server so they show up instantly.
    let dev_guild = std::env::var("GUILD_ID")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .map(serenity::GuildId::new);

    let options = poise::FrameworkOptions {
        commands: commands::all(),
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(prefix.clone()),
            mention_as_prefix: true,
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |ctx, event, _framework, data| Box::pin(events::handle(ctx, event, data)),
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(options)
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;
                match dev_guild {
                    Some(guild) => poise::builtins::register_in_guild(ctx, commands, guild).await?,
                    None => poise::builtins::register_globally(ctx, commands).await?,
                }
                ctx.set_activity(Some(serenity::ActivityData::listening(format!("{prefix}play"))));
                println!("Logged in as {}", ready.user.name);
                Ok(Data::new())
            })
        })
        .build();

    // Message Content must be switched on in the developer portal (Bot tab) for prefix commands.
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .await?
        .start()
        .await?;
    Ok(())
}

/// Turns errors into friendly replies instead of silent failures.
async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    use poise::FrameworkError as E;
    match error {
        E::Command { error, ctx, .. } => util::failure(ctx, error.to_string()).await,
        E::ArgumentParse { ctx, input, .. } => {
            let usage = format!("{}help {}", ctx.prefix(), ctx.command().qualified_name);
            let what = input.map(|i| format!(" `{i}`")).unwrap_or_default();
            util::failure(ctx, format!("Couldn't understand{what}. See `{usage}`.")).await;
        }
        E::GuildOnly { ctx, .. } => util::failure(ctx, "This only works in a server.").await,
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                eprintln!("Error while handling an error: {e}");
            }
        }
    }
}
