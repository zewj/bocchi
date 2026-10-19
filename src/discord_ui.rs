//! The music cards in Discord: the "Now playing" card with a live progress bar and control
//! buttons, and the setlist (queue) with page buttons.

use crate::data::{Data, Error, LoopMode, TrackInfo};
use crate::emojis::{self, ButtonIcon};
use crate::player::{self, Shared};
use crate::util;
use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ChannelId, ComponentInteraction, CreateActionRow, CreateButton,
    CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditMessage, GuildId, MessageId,
};
use rand::seq::SliceRandom;
use songbird::tracks::PlayMode;
use std::time::Duration;

/// How often the "Now playing" card's progress bar refreshes.
const LIVE_UPDATE: Duration = Duration::from_secs(15);
const PAGE_SIZE: usize = 10;
const YELLOW: u32 = 0xF7D44C;

/// Every button ID starts with this, e.g. `np:skip` or `np:page:2`.
const PREFIX: &str = "np:";

pub fn owns(custom_id: &str) -> bool {
    custom_id.starts_with(PREFIX)
}

// ============================== Now playing card ==============================

/// The card for whatever is playing in a server, or None if nothing is.
pub async fn now_playing_card(shared: &Shared, guild: GuildId) -> Option<(CreateEmbed, Vec<CreateActionRow>)> {
    let call = shared.songbird.get(guild)?;
    let (current, next, waiting) = {
        let call = call.lock().await;
        let queue = call.queue().current_queue();
        let next = queue.get(1).map(|h| (*player::info(h)).clone());
        (call.queue().current()?, next, queue.len().saturating_sub(1))
    };
    let state = current.get_info().await.ok()?;
    let paused = state.playing == PlayMode::Pause;
    let song = player::info(&current);
    let (volume, loop_mode) = shared.state(guild, |s| (s.volume, s.loop_mode));

    let heading = if paused { "📦 Paused... hiding in my box" } else { "🎸 N-now playing..." };
    let progress = format!(
        "`{}` {} `{}`",
        util::format_time(state.position),
        bar(state.position, song.duration),
        util::format_length(song.duration)
    );
    let up_next = match &next {
        Some(next) => format!("[{}]({})", clip(&next.title, 40), next.url),
        None => "Nothing yet...".to_string(),
    };
    let loop_text = match loop_mode {
        LoopMode::Off => "Loop off",
        LoopMode::Song => "Looping this song",
        LoopMode::Queue => "Looping the setlist",
    };

    let mut embed = CreateEmbed::new()
        .color(if paused { YELLOW } else { util::ACCENT })
        .author(CreateEmbedAuthor::new(heading))
        .title(&song.title)
        .url(&song.url)
        .description(progress)
        .field("Requested by", format!("<@{}>", song.requester), true)
        .field("Up next", up_next, true)
        .footer(CreateEmbedFooter::new(format!(
            "Volume {:.0}% · {loop_text} · {waiting} in the setlist",
            volume * 100.0
        )));
    if let Some(thumbnail) = util::thumbnail(&song) {
        embed = embed.thumbnail(thumbnail);
    }
    Some((embed, controls(paused, loop_mode)))
}

/// ━━━━━●──────── like a music player.
fn bar(position: Duration, length: Option<Duration>) -> String {
    const WIDTH: usize = 16;
    let Some(length) = length.filter(|l| !l.is_zero()) else { return "🔴 LIVE".into() };
    let done = ((position.as_secs_f64() / length.as_secs_f64()) * WIDTH as f64) as usize;
    let done = done.min(WIDTH - 1);
    format!("{}●{}", "━".repeat(done), "─".repeat(WIDTH - 1 - done))
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    format!("{}…", text.chars().take(max - 1).collect::<String>())
}

fn icon_button(id: &str, icon: ButtonIcon) -> CreateButton {
    CreateButton::new(format!("{PREFIX}{id}")).emoji(emojis::get(icon)).style(ButtonStyle::Secondary)
}

fn controls(paused: bool, loop_mode: LoopMode) -> Vec<CreateActionRow> {
    let toggle = if paused { ButtonIcon::Play } else { ButtonIcon::Pause };
    let loop_icon = if loop_mode == LoopMode::Song { ButtonIcon::LoopOne } else { ButtonIcon::Loop };
    let loop_style = if loop_mode == LoopMode::Off { ButtonStyle::Secondary } else { ButtonStyle::Primary };
    vec![
        CreateActionRow::Buttons(vec![
            icon_button("back", ButtonIcon::Back),
            icon_button("toggle", toggle).style(ButtonStyle::Primary),
            icon_button("skip", ButtonIcon::Skip),
            icon_button("shuffle", ButtonIcon::Shuffle),
            icon_button("loop", loop_icon).style(loop_style),
        ]),
        CreateActionRow::Buttons(vec![
            icon_button("queue", ButtonIcon::Queue).label("Setlist"),
            icon_button("stop", ButtonIcon::Stop).label("Stop"),
        ]),
    ]
}

/// Posts the card for a new song, removes the previous one, and keeps the progress bar moving.
pub async fn post_now_playing(shared: Shared, guild: GuildId) {
    let (channel, previous) = shared.state(guild, |s| (s.text_channel, s.now_playing.take()));
    let Some(channel) = channel else { return };
    // Only the newest card is kept, so the chat doesn't fill up with old ones.
    if let Some((old_channel, old_message)) = previous {
        let _ = old_channel.delete_message(&shared.http, old_message).await;
    }
    let Some((embed, rows)) = now_playing_card(&shared, guild).await else { return };
    let Ok(message) = channel.send_message(&shared.http, CreateMessage::new().embed(embed).components(rows)).await else {
        return;
    };
    shared.state(guild, |s| s.now_playing = Some((channel, message.id)));

    tokio::spawn(keep_updated(shared, guild, channel, message.id));
}

/// Refreshes the card until it's replaced or the music stops.
async fn keep_updated(shared: Shared, guild: GuildId, channel: ChannelId, message: MessageId) {
    loop {
        tokio::time::sleep(LIVE_UPDATE).await;
        if shared.state(guild, |s| s.now_playing) != Some((channel, message)) {
            return; // a newer card took over
        }
        let Some((embed, rows)) = now_playing_card(&shared, guild).await else { return };
        let edit = EditMessage::new().embed(embed).components(rows);
        if channel.edit_message(&shared.http, message, edit).await.is_err() {
            return; // deleted
        }
    }
}

/// Turns the last card into a plain "finished" card without buttons.
pub async fn finish(shared: &Shared, guild: GuildId, text: &str) {
    let Some((channel, message)) = shared.state(guild, |s| s.now_playing.take()) else { return };
    let embed = util::embed().author(CreateEmbedAuthor::new(text));
    let _ = channel.edit_message(&shared.http, message, EditMessage::new().embed(embed).components(vec![])).await;
}

// ============================== Setlist pages ==============================

/// One page of the setlist, with ◀ ▶ buttons.
pub async fn setlist_page(shared: &Shared, guild: GuildId, page: usize) -> (CreateEmbed, Vec<CreateActionRow>) {
    let songs: Vec<TrackInfo> = match shared.songbird.get(guild) {
        Some(call) => call.lock().await.queue().current_queue().iter().map(|h| (*player::info(h)).clone()).collect(),
        None => Vec::new(),
    };
    let Some((now, upcoming)) = songs.split_first() else {
        return (util::embed().description("The setlist is empty... add something with `play`, p-please?"), vec![]);
    };

    let pages = upcoming.len().div_ceil(PAGE_SIZE).max(1);
    let page = page.clamp(1, pages);
    let lines: Vec<String> = upcoming
        .iter()
        .enumerate()
        .skip((page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|(i, song)| {
            format!("`{:>2}.` [{}]({}) · `{}`", i + 1, clip(&song.title, 50), song.url, util::format_length(song.duration))
        })
        .collect();
    let up_next = if lines.is_empty() { "Nothing yet. Add more with `play`.".to_string() } else { lines.join("\n") };
    let total: Duration = songs.iter().filter_map(|s| s.duration).sum();

    let mut embed = util::embed()
        .author(CreateEmbedAuthor::new("🎸 Setlist"))
        .description(format!("**Now:** [{}]({})\n\n{up_next}", now.title, now.url))
        .footer(CreateEmbedFooter::new(format!(
            "Page {page}/{pages} · {} songs · {} total",
            songs.len(),
            util::format_time(total)
        )));
    if let Some(thumbnail) = util::thumbnail(now) {
        embed = embed.thumbnail(thumbnail);
    }

    let rows = if pages > 1 {
        vec![CreateActionRow::Buttons(vec![
            icon_button(&format!("page:{}", page - 1), ButtonIcon::Left).disabled(page == 1),
            CreateButton::new(format!("{PREFIX}noop")).label(format!("{page} / {pages}")).style(ButtonStyle::Secondary).disabled(true),
            icon_button(&format!("page:{}", page + 1), ButtonIcon::Right).disabled(page == pages),
        ])]
    } else {
        vec![]
    };
    (embed, rows)
}

// ============================== Button presses ==============================

pub async fn on_button(ctx: &serenity::Context, data: &Data, press: &ComponentInteraction) -> Result<(), Error> {
    let Some(guild) = press.guild_id else { return Ok(()) };
    let Some(songbird) = songbird::get(ctx).await else { return Ok(()) };
    let shared = Shared::new(ctx.http.clone(), songbird.clone(), data.guilds.clone());
    let action = press.data.custom_id.trim_start_matches(PREFIX);

    // Flipping setlist pages is fine for anyone.
    if let Some(page) = action.strip_prefix("page:") {
        let (embed, rows) = setlist_page(&shared, guild, page.parse().unwrap_or(1)).await;
        let update = CreateInteractionResponseMessage::new().embed(embed).components(rows);
        press.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(update)).await?;
        return Ok(());
    }
    if action == "queue" {
        let (embed, rows) = setlist_page(&shared, guild, 1).await;
        let reply = CreateInteractionResponseMessage::new().embed(embed).components(rows).ephemeral(true);
        press.create_response(&ctx.http, CreateInteractionResponse::Message(reply)).await?;
        return Ok(());
    }

    // Controlling the music needs you in the same voice channel.
    let bot_channel = player::bot_voice_channel(ctx, guild);
    let user_channel = ctx.cache.guild(guild).and_then(|g| g.voice_states.get(&press.user.id)?.channel_id);
    let Some(call) = songbird.get(guild).filter(|_| bot_channel.is_some() && bot_channel == user_channel) else {
        return whisper(ctx, press, "J-join my voice channel first... then you can use the buttons.").await;
    };

    match action {
        "toggle" => {
            let call = call.lock().await;
            let current = call.queue().current().ok_or("Nothing is playing.")?;
            if current.get_info().await?.playing == PlayMode::Pause {
                call.queue().resume()?;
            } else {
                call.queue().pause()?;
            }
        }
        "back" => {
            let current = call.lock().await.queue().current().ok_or("Nothing is playing.")?;
            let _ = current.seek_async(Duration::ZERO).await;
        }
        "shuffle" => {
            call.lock().await.queue().modify_queue(|queue| {
                if queue.len() > 2 {
                    let mut upcoming: Vec<_> = queue.drain(1..).collect();
                    upcoming.shuffle(&mut rand::rng());
                    queue.extend(upcoming);
                }
            });
        }
        "loop" => {
            let next = shared.state(guild, |s| {
                s.loop_mode = match s.loop_mode {
                    LoopMode::Off => LoopMode::Queue,
                    LoopMode::Queue => LoopMode::Song,
                    LoopMode::Song => LoopMode::Off,
                };
                s.loop_mode
            });
            if let Some(current) = call.lock().await.queue().current() {
                let _ = if next == LoopMode::Song { current.enable_loop() } else { current.disable_loop() };
            }
        }
        "skip" => {
            // The next song posts its own card and removes this one, so just acknowledge.
            press.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?;
            call.lock().await.queue().skip()?;
            return Ok(());
        }
        "stop" => {
            call.lock().await.queue().stop();
            let embed = util::embed().author(CreateEmbedAuthor::new(format!(
                "⏹️ {} stopped the music. Th-the concert's over...",
                press.user.display_name()
            )));
            shared.state(guild, |s| s.now_playing = None);
            let update = CreateInteractionResponseMessage::new().embed(embed).components(vec![]);
            press.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(update)).await?;
            return Ok(());
        }
        _ => return Ok(()),
    }

    // Show the change right away on the card that was clicked.
    match now_playing_card(&shared, guild).await {
        Some((embed, rows)) => {
            let update = CreateInteractionResponseMessage::new().embed(embed).components(rows);
            press.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(update)).await?;
        }
        None => press.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?,
    }
    Ok(())
}

/// A reply only the person who clicked can see.
async fn whisper(ctx: &serenity::Context, press: &ComponentInteraction, text: &str) -> Result<(), Error> {
    let reply = CreateInteractionResponseMessage::new().content(text).ephemeral(true);
    press.create_response(&ctx.http, CreateInteractionResponse::Message(reply)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar() {
        let bar = bar(Duration::from_secs(30), Some(Duration::from_secs(120)));
        assert_eq!(bar.chars().count(), 16);
        assert!(bar.starts_with("━━━━●"));
        assert_eq!(clip("hello world", 6), "hello…");
    }
}
