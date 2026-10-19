# Bocchi

A Discord music bot written in Rust ([poise](https://github.com/serenity-rs/poise) +
[songbird](https://github.com/serenity-rs/songbird)). Plays from YouTube, SoundCloud and anything else
[yt-dlp](https://github.com/yt-dlp/yt-dlp) supports. Search by name or paste a link or playlist.

Every command works with the prefix (default `!`) and as a slash command.

## Setup

1. Needs `yt-dlp` on PATH. The bot updates it on start and every hour after (YouTube blocks old versions).
2. In the [developer portal](https://discord.com/developers/applications), make a **new** application:
   - **Bot** tab: copy the token, turn on **Message Content Intent**
3. Put the token in `.env` as `DISCORD_TOKEN=...`. Optional: `GUILD_ID` (instant slash commands) and `PREFIX`.
4. Invite with scopes `bot` + `applications.commands` and permissions: View Channels, Send Messages,
   Embed Links, Connect, Speak.
5. Run `run.bat`.

## Commands

| Category | Commands |
|----------|----------|
| Playing  | `play` (`p`) `playnext` `pause` `resume` `skip` (`s`) `skipto` `stop` `seek 1:30` `replay` |
| Queue    | `queue` (`q`) `nowplaying` (`np`) `shuffle` `remove` `clear` `loop off/song/queue` |
| General  | `join` `leave` (`dc`) `volume 0-200` `help` `ping` |

The bot leaves after 5 minutes with nothing playing, or 1 minute after everyone leaves the channel.
Spotify links can't be played directly (search the song name instead). Playlists are capped at 100 songs.

## Project layout

```
src/
  main.rs        startup, error replies
  data.rs        shared state: per-server volume, loop mode, song info
  source.rs      links / searches / playlists -> songs (yt-dlp)
  player.rs      joining voice, the queue, "now playing", auto-leave when idle
  events.rs      leave when everyone else leaves
  util.rs        embeds, time formatting, progress bar
  commands/      playback, queue, general
```
