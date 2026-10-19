//! Small helpers: embeds, time formatting, progress bars, thumbnails.

use crate::data::{Context, Error, TrackInfo};
use poise::serenity_prelude::CreateEmbed;
use std::time::Duration;

/// Bocchi's pink.
pub const ACCENT: u32 = 0xF4A7BB;
pub const FAILURE: u32 = 0xED4245;

pub fn embed() -> CreateEmbed {
    CreateEmbed::new().color(ACCENT)
}

/// Replies with a short pink embed.
pub async fn say(ctx: Context<'_>, text: impl Into<String>) -> Result<(), Error> {
    ctx.send(poise::CreateReply::default().embed(embed().description(text.into()))).await?;
    Ok(())
}

/// Replies with a red "❌ ..." embed that only the user sees (when using slash commands).
pub async fn failure(ctx: Context<'_>, text: impl Into<String>) {
    let embed = CreateEmbed::new().color(FAILURE).description(format!("❌ {}", text.into()));
    let _ = ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true)).await;
}

/// 3725s -> "1:02:05", 95s -> "1:35"
pub fn format_time(time: Duration) -> String {
    let secs = time.as_secs();
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

pub fn format_length(duration: Option<Duration>) -> String {
    duration.map(format_time).unwrap_or_else(|| "live".into())
}

/// Reads `90`, `1:30`, `1:02:30` or `1m30s` as a time.
pub fn parse_time(input: &str) -> Option<Duration> {
    let input = input.trim().to_lowercase();
    if input.contains(':') {
        let mut secs = 0u64;
        for part in input.split(':') {
            secs = secs * 60 + part.parse::<u64>().ok()?;
        }
        return Some(Duration::from_secs(secs));
    }
    let mut secs = 0u64;
    let mut number = String::new();
    for c in input.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let n: u64 = number.parse().ok()?;
        number.clear();
        secs += n * match c {
            'h' => 3600,
            'm' => 60,
            's' => 1,
            _ => return None,
        };
    }
    if !number.is_empty() {
        secs += number.parse::<u64>().ok()?;
    }
    Some(Duration::from_secs(secs))
}

/// The card shown for "Now playing" and `play`.
pub fn song_embed(heading: &str, song: &TrackInfo) -> CreateEmbed {
    let mut embed = embed()
        .author(poise::serenity_prelude::CreateEmbedAuthor::new(heading))
        .title(&song.title)
        .url(&song.url)
        .field("Length", format_length(song.duration), true)
        .field("Requested by", format!("<@{}>", song.requester), true);
    if let Some(thumbnail) = thumbnail(song) {
        embed = embed.thumbnail(thumbnail);
    }
    embed
}

/// The song's cover picture. YouTube's is built from the video ID, so playlist songs get one too.
pub fn thumbnail(song: &TrackInfo) -> Option<String> {
    youtube_id(&song.url)
        .map(|id| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"))
        .or_else(|| song.thumbnail.clone())
}

fn youtube_id(url: &str) -> Option<&str> {
    let rest = url
        .split_once("watch?v=")
        .or_else(|| url.split_once("youtu.be/"))
        .or_else(|| url.split_once("/shorts/"))?
        .1;
    let id = rest.split(['&', '?', '/']).next()?;
    (id.len() == 11).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("1:30"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("1:02:30"), Some(Duration::from_secs(3750)));
        assert_eq!(parse_time("1m30s"), Some(Duration::from_secs(90)));
        assert_eq!(parse_time("abc"), None);
        assert_eq!(format_time(Duration::from_secs(95)), "1:35");
        assert_eq!(format_time(Duration::from_secs(3725)), "1:02:05");
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1"), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id("https://youtu.be/dQw4w9WgXcQ"), Some("dQw4w9WgXcQ"));
        assert_eq!(youtube_id("https://soundcloud.com/a/b"), None);
    }
}
