//! Bocchi: a Discord music bot with a desktop control panel. 🎸
//! Audio comes from yt-dlp (YouTube, SoundCloud and more) and is played through songbird.
//!
//! Run normally for the control panel window, or with `--headless` to run without a window
//! (the log then goes to bocchi.log).
//!
//! The console-only version leaves the window out entirely, so it uses far less memory:
//!   cargo build --release --no-default-features
//! It prints the log live and stops with Ctrl+C.

// No console window behind the control panel in release builds (the console version keeps it).
#![cfg_attr(all(not(debug_assertions), feature = "gui"), windows_subsystem = "windows")]
// Some details (bot status, settings saving) are only read by the control panel.
#![cfg_attr(not(feature = "gui"), allow(dead_code))]

mod bot;
mod commands;
mod config;
#[cfg(feature = "gui")]
mod control;
mod data;
mod discord_ui;
mod emojis;
mod events;
#[cfg(feature = "gui")]
mod gui;
mod log;
mod player;
mod process;
mod source;
mod spotify;
#[cfg(feature = "gui")]
mod theme;
#[cfg(feature = "gui")]
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

    #[cfg(feature = "gui")]
    if !std::env::args().any(|arg| arg == "--headless") {
        return gui::run(runtime.handle().clone());
    }
    runtime.block_on(headless())
}

/// Runs the bot without a window until Ctrl+C.
async fn headless() -> Result<(), Error> {
    let config = config::Config::load();
    if !config.has_token() {
        log!("No bot token. Put DISCORD_TOKEN=... in a .env file next to this program.");
        return Err("missing DISCORD_TOKEN".into());
    }
    log!("Bocchi (console) starting. Press Ctrl+C to stop.");
    let status = std::sync::Arc::new(std::sync::Mutex::new(bot::Status::Offline));
    let bot = bot::start(config, Default::default(), status.clone()).await?;

    // Run until Ctrl+C, or until Discord disconnects us for good (e.g. a bad token).
    let mut check = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = check.tick() => {
                if let bot::Status::Failed(reason) = &*status.lock().unwrap() {
                    return Err(reason.clone().into());
                }
            }
        }
    }
    log!("Stopping...");
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
