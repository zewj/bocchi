//! Turns what the user typed (a link or a search) into songs, and streams them, using yt-dlp.

use crate::data::{Error, TrackInfo};
use crate::process;
use async_trait::async_trait;
use poise::serenity_prelude::UserId;
use serde_json::Value;
use songbird::input::core::io::{MediaSource, ReadOnlySource};
use songbird::input::{AudioStream, AudioStreamError, AuxMetadata, ChildContainer, Compose, Input};
use std::process::Stdio;
use std::time::Duration;

/// Playlists longer than this are cut off.
const MAX_PLAYLIST: usize = 100;

pub fn is_url(query: &str) -> bool {
    query.starts_with("http://") || query.starts_with("https://")
}

fn is_playlist(url: &str) -> bool {
    url.contains("list=") || url.contains("/playlist") || url.contains("/sets/")
}

/// Finds the song(s) for a link or search. Playlists give many songs.
pub async fn resolve(query: &str, requester: UserId) -> Result<Vec<TrackInfo>, Error> {
    let query = query.trim();
    if query.contains("spotify.com") {
        return Err("Spotify links can't be played directly. Search the song name instead.".into());
    }
    if is_url(query) && is_playlist(query) {
        return playlist(query, requester).await;
    }

    // A search becomes "the first YouTube result".
    let target = if is_url(query) { query.to_string() } else { format!("ytsearch1:{query}") };
    let json = ytdlp_json(&["--dump-json", "--no-playlist", &target])
        .await
        .map_err(|_| format!("Couldn't find anything for `{query}`."))?;

    Ok(vec![TrackInfo {
        title: json["title"].as_str().unwrap_or("Unknown title").to_string(),
        url: json["webpage_url"].as_str().unwrap_or(query).to_string(),
        duration: json["duration"].as_f64().map(Duration::from_secs_f64),
        thumbnail: json["thumbnail"].as_str().map(str::to_string),
        requester,
    }])
}

/// Lists a playlist's songs without downloading anything ("flat" mode is fast).
async fn playlist(url: &str, requester: UserId) -> Result<Vec<TrackInfo>, Error> {
    let json = ytdlp_json(&["--flat-playlist", "--dump-single-json", url])
        .await
        .map_err(|_| "Couldn't read that playlist.")?;
    let entries = json["entries"].as_array().ok_or("That playlist is empty or private.")?;

    let songs: Vec<TrackInfo> = entries
        .iter()
        .take(MAX_PLAYLIST)
        .filter_map(|entry| {
            // YouTube gives a full URL; some sites only give an ID.
            let url = entry["url"]
                .as_str()
                .map(str::to_string)
                .or_else(|| entry["id"].as_str().map(|id| format!("https://www.youtube.com/watch?v={id}")))?;
            Some(TrackInfo {
                title: entry["title"].as_str().unwrap_or("Unknown title").to_string(),
                url,
                duration: entry["duration"].as_f64().map(Duration::from_secs_f64),
                thumbnail: None,
                requester,
            })
        })
        .collect();

    if songs.is_empty() {
        return Err("That playlist is empty or private.".into());
    }
    Ok(songs)
}

/// Runs yt-dlp and reads the JSON it prints. With searches it prints one line per result.
async fn ytdlp_json(args: &[&str]) -> Result<Value, Error> {
    let output = process::async_command("yt-dlp")
        .args(["--no-warnings", "--format", "bestaudio/best"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|_| "yt-dlp isn't installed or isn't on PATH.")?;
    let first_line = output.stdout.split(|b| *b == b'\n').find(|l| !l.is_empty()).ok_or("no results")?;
    Ok(serde_json::from_slice(first_line)?)
}

// ============================== Audio stream ==============================

/// Streams a song by letting yt-dlp download it and pipe the audio to us.
///
/// songbird's built-in YoutubeDl source downloads the audio itself, and YouTube answers that
/// with "403 Forbidden". yt-dlp's own downloader knows how to get the audio, so we use it.
pub struct YtDlpStream {
    url: String,
    duration: Option<Duration>,
}

impl YtDlpStream {
    /// Lazy: yt-dlp only starts when the song is about to play (and again for seeking back).
    /// Knowing the length lets songbird start the *next* song 5 seconds early, so there's
    /// no gap between songs.
    pub fn input(url: String, duration: Option<Duration>) -> Input {
        Input::Lazy(Box::new(Self { url, duration }))
    }
}

#[async_trait]
impl Compose for YtDlpStream {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let child = process::command("yt-dlp")
            .args(["--quiet", "--no-warnings", "--no-playlist"])
            .args(["--format", "bestaudio[acodec=opus]/bestaudio/best"])
            .args(["--output", "-", &self.url])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AudioStreamError::Fail(format!("couldn't start yt-dlp: {e}").into()))?;
        let source = ReadOnlySource::new(ChildContainer::from(child));
        Ok(AudioStream { input: Box::new(source) as Box<dyn MediaSource> })
    }

    async fn create_async(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    fn should_create_async(&self) -> bool {
        false
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        // songbird asks for this to schedule the next song's preload.
        Ok(AuxMetadata { duration: self.duration, source_url: Some(self.url.clone()), ..Default::default() })
    }
}
