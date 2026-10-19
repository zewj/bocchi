//! First-run setup, so the exe works on a fresh PC with nothing installed:
//! - downloads yt-dlp and Deno (YouTube needs it) into a `tools` folder next to the exe
//! - in the console version, asks for the bot token if there isn't one yet

use crate::config::Config;
use crate::data::Error;
use crate::log;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const DENO_URL: &str = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

/// The folder the exe is in.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `tools` next to the exe.
pub fn tools_dir() -> PathBuf {
    exe_dir().join("tools")
}

/// Puts the tools folder first on PATH, so our yt-dlp is used and it can find our Deno.
/// Call this first thing in `main`, before any other threads exist.
pub fn add_tools_to_path() {
    let mut paths = vec![tools_dir()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        // SAFETY: called at the very start of main, while only one thread is running.
        unsafe { std::env::set_var("PATH", joined) };
    }
}

/// Whether a program can be found on PATH (which includes the tools folder).
fn installed(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&paths).any(|dir| dir.join(format!("{program}.exe")).is_file())
}

/// Downloads yt-dlp and Deno if they're missing. Already-installed copies are left alone.
pub async fn ensure_tools() -> Result<(), Error> {
    let dir = tools_dir();
    if !installed("yt-dlp") {
        std::fs::create_dir_all(&dir)?;
        log!("yt-dlp isn't installed, downloading it (about 18 MB)...");
        download(YTDLP_URL, &dir.join("yt-dlp.exe")).await?;
        log!("yt-dlp is ready");
    }
    if !installed("deno") {
        std::fs::create_dir_all(&dir)?;
        log!("Deno isn't installed, downloading it (about 43 MB, YouTube needs it)...");
        let zip = dir.join("deno.zip");
        download(DENO_URL, &zip).await?;
        let target = dir.join("deno.exe");
        tokio::task::spawn_blocking(move || unzip_one(&zip, "deno.exe", &target)).await??;
        log!("Deno is ready");
    }
    Ok(())
}

/// Saves a URL to a file, logging progress. Writes to `<file>.part` first so a cut-off
/// download never looks finished.
async fn download(url: &str, path: &Path) -> Result<(), Error> {
    let mut response = reqwest::get(url).await?.error_for_status()?;
    let total = response.content_length().unwrap_or(0);
    let partial = path.with_extension("part");
    let mut file = std::fs::File::create(&partial)?;

    let (mut done, mut next_report) = (0u64, 25u64);
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
        done += chunk.len() as u64;
        if total > 0 && done * 100 / total >= next_report {
            log!("  ...{next_report}%");
            next_report += 25;
        }
    }
    drop(file);
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// Pulls one file out of a zip, then deletes the zip.
fn unzip_one(zip_path: &Path, name: &str, target: &Path) -> Result<(), Error> {
    {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
        let mut entry = archive.by_name(name)?;
        let mut out = std::fs::File::create(target)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    std::fs::remove_file(zip_path)?;
    Ok(())
}

/// Console version only: if there's no token yet, explain how to get one and ask for it.
/// Returns the config with the token filled in (and saved to `.env`).
pub fn ask_for_token(mut config: Config) -> Result<Config, Error> {
    if config.has_token() {
        return Ok(config);
    }
    if !std::io::stdin().is_terminal() {
        return Err("No bot token. Put DISCORD_TOKEN=... in a .env file next to this program.".into());
    }

    println!();
    println!("  Bocchi needs a Discord bot to log in as. One-time setup:");
    println!();
    println!("  1. Open https://discord.com/developers/applications and click New Application.");
    println!("  2. Go to the Bot tab, click Reset Token and copy it.");
    println!("  3. On the same tab, turn ON \"Message Content Intent\" and save.");
    println!();
    println!("  The token is saved to .env next to this program. Keep that file private:");
    println!("  anyone with the token can control your bot.");
    println!();
    print!("  Paste your bot token: ");
    std::io::stdout().flush()?;

    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;
    config.token = token.trim().trim_matches('"').to_string();
    if !config.has_token() {
        return Err("No token entered.".into());
    }
    config.save()?;
    println!("  Saved! Starting Bocchi...");
    println!();
    Ok(config)
}
