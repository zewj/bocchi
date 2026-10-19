//! Turns what the user typed (a link or a search) into songs, using yt-dlp.

use crate::data::{Error, TrackInfo};
use poise::serenity_prelude::UserId;
use songbird::input::{Compose, YoutubeDl};
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
pub async fn resolve(http: &reqwest::Client, query: &str, requester: UserId) -> Result<Vec<TrackInfo>, Error> {
    let query = query.trim();
    if query.contains("spotify.com") {
        return Err("Spotify links can't be played directly. Search the song name instead.".into());
    }
    if is_url(query) && is_playlist(query) {
        return playlist(query, requester).await;
    }

    let mut source = if is_url(query) {
        YoutubeDl::new(http.clone(), query.to_string())
    } else {
        YoutubeDl::new_search(http.clone(), query.to_string())
    };
    let meta = source
        .aux_metadata()
        .await
        .map_err(|_| format!("Couldn't find anything for `{query}`."))?;

    Ok(vec![TrackInfo {
        title: meta.title.unwrap_or_else(|| "Unknown title".into()),
        url: meta.source_url.unwrap_or_else(|| query.to_string()),
        duration: meta.duration,
        thumbnail: meta.thumbnail,
        requester,
    }])
}

/// Lists a playlist's songs without downloading anything ("flat" mode is fast).
async fn playlist(url: &str, requester: UserId) -> Result<Vec<TrackInfo>, Error> {
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--flat-playlist", "--dump-single-json", "--no-warnings", url])
        .output()
        .await
        .map_err(|_| "yt-dlp isn't installed or isn't on PATH.")?;
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "Couldn't read that playlist.")?;
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
