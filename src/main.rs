//! Bocchi: a Discord music bot with a desktop control panel. 🎸
//! Audio comes from yt-dlp (YouTube, SoundCloud and more) and is played through songbird.
//!
//! Run normally for the control panel window, or with `--headless` to run without a window
//! (the log then goes to bocchi.log).

// No console window behind the control panel in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bot;
mod commands;
mod config;
mod control;
mod data;
mod discord_ui;
mod emojis;
mod events;
mod gui;
mod log;
mod player;
mod process;
mod source;
mod spotify;
mod theme;
mod widgets;
mod util;

use data::Error;
use std::time::Duration;

/// How often to check for a new yt-dlp.
const YTDLP_UPDATE_EVERY: Duration = Duration::from_secs(60 * 60);

fn main() -> Result<(), Error> {
    // Two worker threads are plenty for a music bot (the default is one per CPU core).
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    let _inside_runtime = runtime.enter();
    keep_ytdlp_updated();

    if std::env::args().any(|arg| arg == "--headless") {
        runtime.block_on(headless())
    } else {
        gui::run(runtime.handle().clone())
    }
}

/// Runs the bot without a window until it's stopped.
async fn headless() -> Result<(), Error> {
    let status = std::sync::Arc::new(std::sync::Mutex::new(bot::Status::Offline));
    let bot = bot::start(config::Config::load(), Default::default(), status).await?;
    tokio::signal::ctrl_c().await?;
    bot.stop().await;
    Ok(())
}

/// YouTube keeps changing, and old yt-dlp versions get blocked (HTTP 403).
/// Checks for an update now and then every hour, in the background, so even a bot that runs
/// for months stays current. Songs already playing keep using the old copy until they end.
fn keep_ytdlp_updated() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(YTDLP_UPDATE_EVERY);
        loop {
            interval.tick().await; // the first tick is immediate
            match process::async_command("yt-dlp").arg("-U").output().await {
                Ok(out) => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    // Only mention it when something actually changed.
                    match text.lines().find(|l| l.starts_with("Updated yt-dlp")) {
                        Some(line) => log!("{}", line.trim()),
                        None => log!("yt-dlp is up to date"),
                    }
                }
                Err(_) => log!("yt-dlp wasn't found on PATH. Music won't play until it's installed."),
            }
        }
    });
}
