# 🎸 Bocchi

A shy Discord music bot themed after *Bocchi the Rock!*, written in Rust ([poise](https://github.com/serenity-rs/poise) +
[songbird](https://github.com/serenity-rs/songbird)). Plays from YouTube, SoundCloud and anything else
[yt-dlp](https://github.com/yt-dlp/yt-dlp) supports. Search by name or paste a link or playlist.

Every command works with the prefix (default `!`) and as a slash command.

## Control panel

Running `bocchi.exe` opens a pink control panel to start/stop the bot, edit settings (saved to `.env`), and control
every server it's playing in: a now-playing card with album art and a click-to-jump progress bar,
pause/skip/replay/shuffle/stop/leave, volume, loop, adding songs, and the setlist (queue) with album art and
move up/down/remove buttons. The log is shown at the bottom and saved to `bocchi.log`.

To run without a window (e.g. as a service): `bocchi.exe --headless`.

## Console-only build

For a machine that just hosts the bot, build without the control panel. The window code isn't
compiled in at all, so it uses much less memory. It shows the log live and stops with Ctrl+C:

```
cargo build --release --no-default-features
```

On first run it sets itself up, so the exe can be given to someone with nothing installed:
- if yt-dlp or Deno aren't installed, it downloads them into a `tools` folder next to the exe
- if there's no token, it explains how to make a bot and asks for the token (saved to `.env`)
- once online, the log shows the invite link with exactly the permissions Bocchi needs

## Setup

1. yt-dlp and Deno: downloaded automatically into `tools/` if they aren't installed. yt-dlp is updated on start
   and every hour after (YouTube blocks old versions).
2. In the [developer portal](https://discord.com/developers/applications), make a **new** application:
   - **Bot** tab: copy the token, turn on **Message Content Intent**
3. Open the app, click **Settings**, paste the token and save (or edit `.env` by hand).
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
Spotify playlist, album and song links work without an API key: Bocchi reads Spotify's public embed page
and finds each song on YouTube Music right before it plays. Playlists are capped at 100 songs.

## Project layout

```
src/
  main.rs        startup (window or --headless), hourly yt-dlp updates
  gui.rs         the control panel window
  theme.rs       Bocchi's colors and the window icon
  control.rs     what the panel shows and its button actions
  bot.rs         starting / stopping the Discord bot
  config.rs      settings in .env
  log.rs         the shared log (window + file)
  process.rs     starting yt-dlp without console windows
  setup.rs       first-run setup: downloads yt-dlp/Deno, asks for the token
  data.rs        shared state: per-server volume, loop mode, song info
  source.rs      links / searches / playlists -> songs (yt-dlp)
  spotify.rs     Spotify links via the public embed page
  player.rs      joining voice, the queue, "now playing", auto-leave when idle
  events.rs      leave when everyone else leaves
  util.rs        embeds, time formatting, progress bar
  commands/      playback, queue, general
```

The title font is [RocknRoll One](https://github.com/fontworks-fonts/RocknRoll) (SIL Open Font License,
see `assets/RocknRollOne-OFL.txt`), trimmed to basic Latin letters.
