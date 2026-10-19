//! Spotify links (playlists, albums, songs) without an API key.
//!
//! Spotify's public embed player page (`open.spotify.com/embed/...`) carries the song list as
//! JSON. We read the titles, artists and lengths from it, then each song is found on YouTube
//! Music right before it plays. Covers come from Spotify's official oEmbed endpoint.
//!
//! The embed page isn't an official API, so if Spotify changes it this may need updating.

use crate::data::{Error, TrackInfo};
use crate::source::{youtube_music_search, MAX_PLAYLIST};
use futures::stream::{self, StreamExt};
use poise::serenity_prelude::UserId;
use serde_json::Value;
use std::time::Duration;

/// Spotify serves the embed page to normal browsers.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
/// How many covers to fetch at once.
const COVER_REQUESTS: usize = 8;

pub fn is_spotify(query: &str) -> bool {
    query.contains("open.spotify.com/") || query.starts_with("spotify:")
}

/// ("playlist" | "album" | "track", id) from a link like
/// `https://open.spotify.com/intl-de/playlist/37i9dQZF1DXcBWIGoYBM5M?si=...` or `spotify:album:ID`.
fn parse(link: &str) -> Option<(&'static str, String)> {
    let path = link.split(['?', '#']).next()?;
    let parts: Vec<&str> = path.split(['/', ':']).filter(|p| !p.is_empty()).collect();
    ["playlist", "album", "track"].into_iter().find_map(|kind| {
        let at = parts.iter().position(|p| *p == kind)?;
        let id = parts.get(at + 1)?;
        (id.len() == 22 && id.chars().all(|c| c.is_ascii_alphanumeric())).then(|| (kind, id.to_string()))
    })
}

pub async fn resolve(link: &str, requester: UserId) -> Result<Vec<TrackInfo>, Error> {
    let (kind, id) = parse(link).ok_or("That doesn't look like a Spotify playlist, album or song link.")?;
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let entity = embed_data(&client, kind, &id).await?;

    // Albums and songs carry their cover; playlists only have the playlist's cover.
    let own_cover = cover_from(&entity);

    let songs: Vec<SpotifySong> = if kind == "track" {
        let artists = entity["artists"]
            .as_array()
            .map(|list| list.iter().filter_map(|a| a["name"].as_str()).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        vec![SpotifySong::new(&entity, artists)]
    } else {
        entity["trackList"]
            .as_array()
            .ok_or("That Spotify link has no songs (is the playlist public?).")?
            .iter()
            .filter(|t| t["entityType"].as_str() != Some("episode"))
            .take(MAX_PLAYLIST)
            // Artists come as "A,\u{a0}B".
            .map(|t| SpotifySong::new(t, t["subtitle"].as_str().unwrap_or("").replace('\u{a0}', " ")))
            .collect()
    };
    if songs.is_empty() {
        return Err("That Spotify link has no songs (is the playlist public?).".into());
    }

    let covers: Vec<Option<String>> = if kind == "playlist" {
        // One small request per song, a few at a time, so each card shows its real cover.
        // Each request owns its client copy (they share one connection pool), so the
        // requests can run on any thread.
        let requests: Vec<_> = songs
            .iter()
            .map(|song| {
                let (client, uri) = (client.clone(), song.uri.clone());
                async move { cover_for(client, uri).await }
            })
            .collect();
        stream::iter(requests).buffered(COVER_REQUESTS).collect().await
    } else {
        vec![own_cover; songs.len()]
    };

    Ok(songs
        .into_iter()
        .zip(covers)
        .map(|(song, cover)| song.into_track(cover, requester))
        .collect())
}

/// The `entity` JSON from the embed page.
async fn embed_data(client: &reqwest::Client, kind: &str, id: &str) -> Result<Value, Error> {
    let html = client
        .get(format!("https://open.spotify.com/embed/{kind}/{id}"))
        .send()
        .await?
        .error_for_status()
        .map_err(|_| "Spotify didn't find that. Is it public?")?
        .text()
        .await?;

    const START: &str = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let json = html
        .split_once(START)
        .and_then(|(_, rest)| rest.split_once("</script>"))
        .map(|(json, _)| json)
        .ok_or("Couldn't read Spotify's page (they may have changed it).")?;
    let data: Value = serde_json::from_str(json)?;
    let entity = data["props"]["pageProps"]["state"]["data"]["entity"].clone();
    if entity.is_null() {
        return Err("Couldn't read that Spotify link. Is it public?".into());
    }
    Ok(entity)
}

fn cover_from(entity: &Value) -> Option<String> {
    let first = |list: &Value| list.as_array()?.first()?["url"].as_str().map(str::to_string);
    first(&entity["visualIdentity"]["image"]).or_else(|| first(&entity["coverArt"]["sources"]))
}

/// A song's cover from Spotify's oEmbed endpoint (the one link previews use).
async fn cover_for(client: reqwest::Client, uri: Option<String>) -> Option<String> {
    let track_id = uri?.rsplit(':').next()?.to_string();
    let link = format!("https://open.spotify.com/track/{track_id}");
    let response: Value = client
        .get("https://open.spotify.com/oembed")
        .query(&[("url", link)])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    response["thumbnail_url"].as_str().map(str::to_string)
}

struct SpotifySong {
    title: String,
    artists: String,
    duration: Option<Duration>,
    /// `spotify:track:ID`, used to look up the cover.
    uri: Option<String>,
}

impl SpotifySong {
    fn new(json: &Value, artists: String) -> Self {
        Self {
            title: json["title"].as_str().or(json["name"].as_str()).unwrap_or("Unknown title").to_string(),
            artists,
            duration: json["duration"].as_u64().filter(|ms| *ms > 0).map(Duration::from_millis),
            uri: json["uri"].as_str().map(str::to_string),
        }
    }

    /// The YouTube Music search for this song becomes its "link"; it's looked up when it plays.
    fn into_track(self, cover: Option<String>, requester: UserId) -> TrackInfo {
        let main_artist = self.artists.split(',').next().unwrap_or("").trim().to_string();
        let title = if main_artist.is_empty() { self.title.clone() } else { format!("{main_artist} - {}", self.title) };
        TrackInfo {
            url: youtube_music_search(&format!("{} {}", self.artists.replace(',', ""), self.title)),
            title,
            duration: self.duration,
            thumbnail: cover,
            requester,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        assert_eq!(
            parse("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc"),
            Some(("playlist", "37i9dQZF1DXcBWIGoYBM5M".to_string()))
        );
        assert_eq!(
            parse("https://open.spotify.com/intl-de/album/4yP0hdKOZPNshxUOjY0cZj"),
            Some(("album", "4yP0hdKOZPNshxUOjY0cZj".to_string()))
        );
        assert_eq!(parse("spotify:track:0VjIjW4GlUZAMYd2vXMi3b"), Some(("track", "0VjIjW4GlUZAMYd2vXMi3b".to_string())));
        assert_eq!(parse("https://open.spotify.com/artist/1Xyo4u8uXC1ZmMpatF05PJ"), None);
    }
}

/// Talks to the real Spotify site, so it only runs when asked: `cargo test -- --ignored`.
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn reads_a_real_playlist() {
        let started = std::time::Instant::now();
        let songs = resolve("https://open.spotify.com/playlist/6UeSakyzhiEt4NB3UAd6NQ", UserId::new(1)).await.unwrap();
        let with_cover = songs.iter().filter(|s| s.thumbnail.is_some()).count();
        println!("{} songs in {:.1}s, {with_cover} with covers", songs.len(), started.elapsed().as_secs_f32());
        for song in songs.iter().take(3) {
            println!("  {} [{:?}] -> {}", song.title, song.duration, song.url);
        }
        assert!(songs.len() >= 50 && with_cover > songs.len() / 2);
    }
}
