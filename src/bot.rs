//! Starting and stopping the Discord bot. Used by both the control panel and headless mode.

use crate::config::Config;
use crate::data::{Data, Error, Guilds};
use crate::player::Shared;
use crate::{commands, events, log, util};
use poise::serenity_prelude::{self as serenity, Cache, GuildId, Http, ShardManager};
use songbird::{SerenityInit, Songbird};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub enum Status {
    Offline,
    Starting,
    Online { name: String, since: Instant },
    Failed(String),
}

pub type StatusLock = Arc<Mutex<Status>>;

fn set(status: &StatusLock, value: Status) {
    *status.lock().unwrap() = value;
}

/// A running bot, with the handles the control panel needs.
pub struct Bot {
    pub http: Arc<Http>,
    pub cache: Arc<Cache>,
    pub songbird: Arc<Songbird>,
    pub guilds: Guilds,
    shard_manager: Arc<ShardManager>,
}

impl Bot {
    pub fn shared(&self) -> Shared {
        Shared::new(self.http.clone(), self.songbird.clone(), self.guilds.clone())
    }

    /// Leaves every voice channel and disconnects from Discord.
    pub async fn stop(&self) {
        let calls: Vec<GuildId> = self.songbird.iter().map(|(guild, _)| GuildId::new(guild.0.get())).collect();
        for guild in calls {
            if let Some(call) = self.songbird.get(guild) {
                call.lock().await.queue().stop();
            }
            let _ = self.songbird.remove(guild).await;
        }
        self.shard_manager.shutdown_all().await;
        log!("Bot stopped");
    }
}

/// Connects to Discord. Returns once the connection is starting; `status` turns Online when ready.
pub async fn start(config: Config, guilds: Guilds, status: StatusLock) -> Result<Bot, Error> {
    if !config.has_token() {
        return Err("No bot token set. Add it in Settings.".into());
    }
    set(&status, Status::Starting);
    log!("Starting...");

    let prefix = if config.prefix.trim().is_empty() { "!".to_string() } else { config.prefix.trim().to_string() };
    let dev_guild = config.guild_id.trim().parse().ok().map(GuildId::new);

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

    let ready_status = status.clone();
    let data_guilds = guilds.clone();
    let framework = poise::Framework::builder()
        .options(options)
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;
                match dev_guild {
                    Some(guild) => poise::builtins::register_in_guild(ctx, commands, guild).await?,
                    None => poise::builtins::register_globally(ctx, commands).await?,
                }
                ctx.set_activity(Some(serenity::ActivityData::playing(format!("guitar in the closet 🎸 {prefix}play"))));
                log!("Online as {} in {} server(s)", ready.user.name, ready.guilds.len());
                set(&ready_status, Status::Online { name: ready.user.name.clone(), since: Instant::now() });
                Ok(Data { guilds: data_guilds })
            })
        })
        .build();

    // Message Content must be switched on in the developer portal (Bot tab) for prefix commands.
    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;
    let songbird = Songbird::serenity();
    let mut client = serenity::ClientBuilder::new(config.token.trim(), intents)
        .framework(framework)
        .register_songbird_with(songbird.clone())
        .await?;

    let bot = Bot {
        http: client.http.clone(),
        cache: client.cache.clone(),
        songbird,
        guilds,
        shard_manager: client.shard_manager.clone(),
    };

    // Runs until stopped (or the token is rejected).
    tokio::spawn(async move {
        match client.start().await {
            Ok(()) => set(&status, Status::Offline),
            Err(e) => {
                let reason = match e.to_string() {
                    text if text.contains("Authentication") || text.contains("401") => "Discord rejected the token. Check it in Settings.".to_string(),
                    text if text.contains("Disallowed") => {
                        "Turn on Message Content Intent in the developer portal (Bot tab).".to_string()
                    }
                    text => text,
                };
                log!("Bot stopped: {reason}");
                set(&status, Status::Failed(reason));
            }
        }
    });
    Ok(bot)
}

/// Turns command errors into friendly replies instead of silent failures.
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
                log!("Error while handling an error: {e}");
            }
        }
    }
}
