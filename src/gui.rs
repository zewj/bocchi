//! The control panel window: start/stop the bot, settings, every server it's playing in,
//! a "now playing" card with album art, playback controls, the setlist (queue), and the log.
//! All buttons and icons are Bocchi's own (see widgets.rs).

use crate::bot::{self, Bot, Status, StatusLock};
use crate::config::Config;
use crate::control::{self, Action, PlayerView};
use crate::data::{Error, Guilds, LoopMode, TrackInfo};
use crate::theme::{self, BLUE, CARD, GREEN, MUTED, PINK, PINK_DEEP, RED, TEXT, YELLOW};
use crate::widgets::{self, button, Icon};
use crate::{log, player, util};
use eframe::egui::{self, Color32, RichText, Sense, Vec2};
use poise::serenity_prelude::GuildId;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;

const REFRESH: Duration = Duration::from_millis(500);
const ART_SIZE: Vec2 = Vec2::new(208.0, 117.0);
const SMALL_ART: Vec2 = Vec2::new(64.0, 36.0);
const ROW_HEIGHT: f32 = 46.0;

pub fn run(runtime: Handle) -> Result<(), Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bocchi")
            .with_icon(theme::icon())
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([780.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Bocchi",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            // Lets egui show album art straight from a URL.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(ControlPanel::new(runtime)))
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

struct ControlPanel {
    runtime: Handle,
    config: Config,
    status: StatusLock,
    guilds: Guilds,
    /// The running bot, if any. Filled in by the start task.
    bot: Arc<Mutex<Option<Arc<Bot>>>>,
    /// Refreshed in the background from the running bot.
    players: Arc<Mutex<Vec<PlayerView>>>,
    selected: Option<GuildId>,
    settings_open: bool,
    show_token: bool,
    add_query: String,
    /// While the volume slider is being dragged, show this instead of the last snapshot.
    volume_drag: Option<f32>,
}

impl ControlPanel {
    fn new(runtime: Handle) -> Self {
        let config = Config::load();
        let panel = Self {
            runtime,
            settings_open: !config.has_token(),
            config,
            status: Arc::new(Mutex::new(Status::Offline)),
            guilds: Default::default(),
            bot: Default::default(),
            players: Default::default(),
            selected: None,
            show_token: false,
            add_query: String::new(),
            volume_drag: None,
        };
        panel.start_refreshing();
        if panel.config.has_token() && panel.config.autostart {
            panel.start_bot();
        }
        panel
    }

    /// Keeps `players` up to date while the bot runs.
    fn start_refreshing(&self) {
        let (bot, players) = (self.bot.clone(), self.players.clone());
        self.runtime.spawn(async move {
            let mut interval = tokio::time::interval(REFRESH);
            loop {
                interval.tick().await;
                let current = bot.lock().unwrap().clone();
                let views = match current {
                    Some(bot) => control::snapshot(&bot).await,
                    None => Vec::new(),
                };
                *players.lock().unwrap() = views;
            }
        });
    }

    fn start_bot(&self) {
        let (config, guilds, status, slot) = (self.config.clone(), self.guilds.clone(), self.status.clone(), self.bot.clone());
        self.runtime.spawn(async move {
            match bot::start(config, guilds, status.clone()).await {
                Ok(bot) => *slot.lock().unwrap() = Some(Arc::new(bot)),
                Err(e) => {
                    log!("Couldn't start: {e}");
                    *status.lock().unwrap() = Status::Failed(e.to_string());
                }
            }
        });
    }

    fn stop_bot(&self) {
        let Some(bot) = self.bot.lock().unwrap().take() else { return };
        let status = self.status.clone();
        self.runtime.spawn(async move {
            bot.stop().await;
            *status.lock().unwrap() = Status::Offline;
        });
    }

    /// Runs a playback action in the background and logs the result.
    fn act(&self, guild: GuildId, action: Action) {
        let Some(bot) = self.bot.lock().unwrap().clone() else { return };
        self.runtime.spawn(async move {
            match control::run(&bot, guild, action).await {
                Ok(message) => log!("{message}"),
                Err(e) => log!("Couldn't do that: {e}"),
            }
        });
    }
}

impl eframe::App for ControlPanel {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Song progress and the log change on their own, so keep redrawing.
        ui.ctx().request_repaint_after(REFRESH);

        let status = self.status.lock().unwrap().clone();
        let players = self.players.lock().unwrap().clone();
        if self.selected.is_none_or(|g| !players.iter().any(|p| p.guild == g)) {
            self.selected = players.first().map(|p| p.guild);
        }

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, &status, &players));
        egui::Panel::bottom("log").resizable(true).default_size(150.0).show(ui, log_panel);
        egui::Panel::left("stages").resizable(true).default_size(230.0).show(ui, |ui| {
            self.server_list(ui, &players, &status);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            match players.iter().find(|p| Some(p.guild) == self.selected) {
                Some(player) => self.player(ui, player),
                None => empty_state(ui, &status, &self.config.prefix),
            }
        });

        let ctx = ui.ctx().clone();
        self.settings_window(&ctx);
    }
}

// ============================== Top bar ==============================

impl ControlPanel {
    fn top_bar(&mut self, ui: &mut egui::Ui, status: &Status, players: &[PlayerView]) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::app_icon(ui, 34.0);
            ui.label(RichText::new("Bocchi").size(26.0).family(theme::title_font()).color(PINK));
            ui.add_space(14.0);

            let (color, text) = match status {
                Status::Offline => (MUTED, "Hiding in the closet".to_string()),
                Status::Starting => (YELLOW, "G-getting ready...".to_string()),
                Status::Online { name, since } => {
                    (GREEN, format!("On stage as {name} · {}", util::format_time(since.elapsed())))
                }
                Status::Failed(reason) => (RED, reason.clone()),
            };
            pill(ui, color, &text);

            if matches!(status, Status::Online { .. }) {
                let played = player::SONGS_PLAYED.load(Ordering::Relaxed);
                let playing = players.iter().filter(|p| p.now.is_some()).count();
                pill(ui, BLUE, &format!("{played} songs played · {playing} playing now"));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(button(Icon::Gear).text("Settings").active(self.settings_open)).clicked() {
                    self.settings_open = !self.settings_open;
                }
                let running = matches!(status, Status::Starting | Status::Online { .. });
                if running {
                    if ui.add(button(Icon::Power).text("Stop").danger()).clicked() {
                        self.stop_bot();
                    }
                } else if ui.add(button(Icon::Power).text("Start").primary()).clicked() {
                    self.start_bot();
                }
            });
        });
        ui.add_space(8.0);
    }

    // ============================== Server list ==============================

    fn server_list(&mut self, ui: &mut egui::Ui, players: &[PlayerView], status: &Status) {
        ui.add_space(8.0);
        ui.label(RichText::new("STAGES").small().strong().color(MUTED));
        ui.add_space(4.0);
        if players.is_empty() {
            let hint = if matches!(status, Status::Online { .. }) {
                "Not playing anywhere yet."
            } else {
                "Start Bocchi to see where she's playing."
            };
            ui.label(RichText::new(hint).color(MUTED));
        }

        for player in players {
            let selected = self.selected == Some(player.guild);
            let (state, color) = match (&player.now, player.paused) {
                (None, _) => ("Idle", MUTED),
                (Some(_), true) => ("Paused", YELLOW),
                (Some(_), false) => ("Playing", GREEN),
            };
            let fill = if selected { PINK_DEEP.gamma_multiply(0.35) } else { CARD };
            let card = egui::Frame::new().fill(fill).corner_radius(10).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::dot(ui, color);
                    ui.label(RichText::new(&player.server).strong().color(TEXT));
                });
                ui.label(RichText::new(format!("{state} in #{}", player.channel)).small().color(MUTED));
                if let Some((song, _)) = &player.now {
                    ui.label(RichText::new(&song.title).small().color(PINK)).on_hover_text(&song.title);
                }
            });
            let card = card.response.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if card.clicked() {
                self.selected = Some(player.guild);
            }
            ui.add_space(4.0);
        }
    }

    // ============================== Player ==============================

    fn player(&mut self, ui: &mut egui::Ui, player: &PlayerView) {
        let guild = player.guild;
        ui.add_space(8.0);
        ui.label(RichText::new(format!("{} · #{}", player.server, player.channel)).color(MUTED));
        ui.add_space(6.0);

        egui::Frame::new().fill(CARD).corner_radius(14).inner_margin(16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.now_playing(ui, player);
        });
        ui.add_space(10.0);

        // Add a song
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.add_query)
                    .hint_text("Add a song: name or link...")
                    .margin(egui::Margin::symmetric(10, 8))
                    .desired_width(ui.available_width() - 90.0),
            );
            let pressed_enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let add = ui.add(button(Icon::Plus).text("Add").primary());
            if (add.clicked() || pressed_enter) && !self.add_query.trim().is_empty() {
                self.act(guild, Action::Add(self.add_query.trim().to_string()));
                self.add_query.clear();
            }
        });
        ui.add_space(12.0);

        self.setlist(ui, player);
    }

    fn now_playing(&mut self, ui: &mut egui::Ui, player: &PlayerView) {
        let guild = player.guild;
        let Some((song, position)) = &player.now else {
            ui.horizontal(|ui| {
                widgets::placeholder(ui, ART_SIZE, Icon::Note);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(32.0);
                    ui.label(RichText::new("Nothing playing...").size(20.0).color(MUTED));
                    ui.label(RichText::new("Add a song below, or use !play in Discord.").color(MUTED));
                });
            });
            return;
        };

        ui.horizontal(|ui| {
            art(ui, util::thumbnail(song), ART_SIZE);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let (heading, color) =
                    if player.paused { ("PAUSED · hiding in the box", YELLOW) } else { ("NOW PLAYING", PINK) };
                ui.label(RichText::new(heading).small().strong().color(color));
                ui.hyperlink_to(RichText::new(&song.title).size(19.0).strong().color(TEXT), &song.url);
                ui.label(RichText::new(format!("requested by {}", requester(player, song))).color(MUTED));
                ui.add_space(10.0);
                self.seek_bar(ui, guild, song, *position);
            });
        });
        ui.add_space(12.0);

        // Spotify-style transport, centered: shuffle, back, big play/pause, forward, loop.
        ui.horizontal(|ui| {
            // Widths of the five buttons below plus spacing, so the row can be centered.
            const ROW_WIDTH: f32 = 30.0 + 34.0 + 48.0 + 34.0 + 30.0 + 4.0 * 8.0 + 2.0 * 10.0;
            // egui centers each button on the row's height so far, so give the row the play
            // button's full height up front; otherwise the buttons before it sit too high.
            ui.set_min_height(48.0);
            ui.add_space(((ui.available_width() - ROW_WIDTH) / 2.0).max(0.0));

            let shuffle = button(Icon::Shuffle).plain().size(18.0).enabled(player.upcoming.len() > 1);
            if ui.add(shuffle).on_hover_text("Shuffle the setlist").clicked() {
                self.act(guild, Action::Shuffle);
            }
            if ui.add(button(Icon::Previous).plain().size(22.0)).on_hover_text("Back to the start").clicked() {
                self.act(guild, Action::Replay);
            }
            ui.add_space(10.0);
            let (icon, action, tip) =
                if player.paused { (Icon::Play, Action::Resume, "Play") } else { (Icon::Pause, Action::Pause, "Pause") };
            if ui.add(button(icon).light().round().size(20.0)).on_hover_text(tip).clicked() {
                self.act(guild, action);
            }
            ui.add_space(10.0);
            if ui.add(button(Icon::Next).plain().size(22.0)).on_hover_text("Skip").clicked() {
                self.act(guild, Action::Skip);
            }
            let (loop_icon, next_mode, tip) = match player.loop_mode {
                LoopMode::Off => (Icon::Loop, LoopMode::Queue, "Loop: off (click to loop the setlist)"),
                LoopMode::Queue => (Icon::Loop, LoopMode::Song, "Loop: setlist (click to loop this song)"),
                LoopMode::Song => (Icon::LoopOne, LoopMode::Off, "Loop: this song (click to turn off)"),
            };
            let looping = player.loop_mode != LoopMode::Off;
            if ui.add(button(loop_icon).plain().size(18.0).active(looping)).on_hover_text(tip).clicked() {
                self.act(guild, Action::Loop(next_mode));
            }
        });
        ui.add_space(6.0);

        // Less common controls on the left, volume on the right.
        ui.horizontal(|ui| {
            if ui.add(button(Icon::Stop).text("Stop")).on_hover_text("Stop and clear the setlist").clicked() {
                self.act(guild, Action::Stop);
            }
            if ui.add(button(Icon::Leave).text("Leave").danger()).on_hover_text("Leave the voice channel").clicked() {
                self.act(guild, Action::Leave);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut volume = self.volume_drag.unwrap_or(player.volume * 100.0);
                let slider = ui.add(egui::Slider::new(&mut volume, 0.0..=200.0).suffix("%").integer().show_value(true));
                self.volume_drag = slider.dragged().then_some(volume);
                if slider.changed() {
                    self.act(guild, Action::Volume(volume / 100.0));
                    // Show the new value right away instead of waiting for the next refresh.
                    if let Some(view) = self.players.lock().unwrap().iter_mut().find(|p| p.guild == guild) {
                        view.volume = volume / 100.0;
                    }
                }
                let volume_icon = if volume < 1.0 { Icon::Mute } else { Icon::Volume };
                widgets::icon(ui, volume_icon, 18.0, MUTED);
            });
        });
    }

    /// A progress bar you can click to jump to that point.
    fn seek_bar(&self, ui: &mut egui::Ui, guild: GuildId, song: &TrackInfo, position: Duration) {
        let fraction = song
            .duration
            .filter(|d| !d.is_zero())
            .map(|d| (position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(util::format_time(position)).small().monospace().color(MUTED));
            let length_text = util::format_length(song.duration);
            let width = ui.available_width() - 50.0;
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::click());
            let hover = ui.ctx().animate_bool(response.id, response.hovered());

            let track = egui::Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 5.0 + 2.0 * hover));
            let painter = ui.painter();
            painter.rect_filled(track, egui::CornerRadius::same(4), Color32::from_rgb(0x3A, 0x31, 0x40));
            let filled = egui::Rect::from_min_size(track.min, Vec2::new(track.width() * fraction, track.height()));
            painter.rect_filled(filled, egui::CornerRadius::same(4), PINK);
            painter.circle_filled(egui::pos2(filled.right(), track.center().y), 5.0 + 2.0 * hover, Color32::WHITE);

            if response.clicked() {
                if let (Some(pointer), Some(length)) = (response.interact_pointer_pos(), song.duration) {
                    let at = ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    self.act(guild, Action::Seek(length.mul_f32(at)));
                }
            }
            response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Click to jump here");
            ui.label(RichText::new(length_text).small().monospace().color(MUTED));
        });
    }

    // ============================== Setlist (queue) ==============================

    fn setlist(&mut self, ui: &mut egui::Ui, player: &PlayerView) {
        let guild = player.guild;
        let total: Duration = player.upcoming.iter().filter_map(|s| s.duration).sum();
        ui.horizontal(|ui| {
            ui.label(RichText::new("SETLIST").small().strong().color(MUTED));
            ui.label(
                RichText::new(format!("{} songs · {}", player.upcoming.len(), util::format_time(total)))
                    .small()
                    .color(MUTED),
            );
        });
        ui.add_space(4.0);

        if player.upcoming.is_empty() {
            ui.label(RichText::new("Nothing up next. Add a song above!").color(MUTED));
            return;
        }

        let count = player.upcoming.len();
        // Only the rows on screen are drawn, so long playlists stay smooth.
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, ROW_HEIGHT, count, |ui, rows| {
            for i in rows {
                let song = &player.upcoming[i];
                let position = i + 1; // position 0 is the song playing now
                ui.horizontal(|ui| {
                    ui.set_height(ROW_HEIGHT - 6.0);
                    ui.label(RichText::new(format!("{position:>3}")).monospace().color(MUTED));
                    art(ui, util::thumbnail(song), SMALL_ART);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&song.title).strong().color(TEXT)).on_hover_text(&song.url);
                        ui.label(
                            RichText::new(format!("{} · {}", util::format_length(song.duration), requester(player, song)))
                                .small()
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(button(Icon::Close).size(12.0).danger()).on_hover_text("Remove").clicked() {
                            self.act(guild, Action::Remove(position));
                        }
                        let down = button(Icon::Down).size(12.0).enabled(position < count);
                        if ui.add(down).on_hover_text("Move down").clicked() {
                            self.act(guild, Action::Move(position, position + 1));
                        }
                        let up = button(Icon::Up).size(12.0).enabled(position > 1);
                        if ui.add(up).on_hover_text("Move up").clicked() {
                            self.act(guild, Action::Move(position, position - 1));
                        }
                    });
                });
            }
        });
    }

    // ============================== Settings ==============================

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        let mut restart = false;
        egui::Window::new("Settings").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                ui.label("Bot token");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.config.token).password(!self.show_token).desired_width(320.0));
                    ui.checkbox(&mut self.show_token, "Show");
                });
                ui.end_row();

                ui.label("Prefix");
                ui.add(egui::TextEdit::singleline(&mut self.config.prefix).desired_width(60.0));
                ui.end_row();

                ui.label("Test server ID");
                ui.add(egui::TextEdit::singleline(&mut self.config.guild_id).hint_text("optional").desired_width(200.0));
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut self.config.autostart, "Start Bocchi when this window opens");
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Token: developer portal → your app → Bot → Reset Token.\nAlso turn on Message Content Intent there.")
                    .small()
                    .color(MUTED),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(button(Icon::Plus).text("Save")).clicked() {
                    match self.config.save() {
                        Ok(()) => log!("Settings saved."),
                        Err(e) => log!("Couldn't save settings: {e}"),
                    }
                }
                if ui.add(button(Icon::Power).text("Save & restart").primary()).clicked() {
                    match self.config.save() {
                        Ok(()) => restart = true,
                        Err(e) => log!("Couldn't save settings: {e}"),
                    }
                }
            });
        });
        self.settings_open = open;

        if restart {
            log!("Settings saved, restarting...");
            self.stop_bot();
            let (config, guilds, status, slot) = (self.config.clone(), self.guilds.clone(), self.status.clone(), self.bot.clone());
            self.runtime.spawn(async move {
                // Give the old connection a moment to close.
                tokio::time::sleep(Duration::from_secs(2)).await;
                match bot::start(config, guilds, status.clone()).await {
                    Ok(bot) => *slot.lock().unwrap() = Some(Arc::new(bot)),
                    Err(e) => *status.lock().unwrap() = Status::Failed(e.to_string()),
                }
            });
        }
    }
}

// ============================== Small pieces ==============================

/// A rounded status badge with a dot.
fn pill(ui: &mut egui::Ui, color: Color32, text: &str) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::dot(ui, color);
                ui.label(RichText::new(text).color(color));
            });
        });
}

/// Album art, or a placeholder when there's none.
fn art(ui: &mut egui::Ui, url: Option<String>, size: Vec2) {
    match url {
        Some(url) => {
            ui.add(egui::Image::new(url).fit_to_exact_size(size).corner_radius(8));
        }
        None => widgets::placeholder(ui, size, Icon::Note),
    }
}

fn requester(player: &PlayerView, song: &TrackInfo) -> String {
    player.names.get(&song.requester).cloned().unwrap_or_else(|| "the panel".into())
}

fn empty_state(ui: &mut egui::Ui, status: &Status, prefix: &str) {
    let (title, detail) = match status {
        Status::Online { .. } => ("Ready to play!".to_string(), format!("Join a voice channel and use {prefix}play in Discord.")),
        Status::Starting => ("G-getting ready...".to_string(), "D-don't look at me...".to_string()),
        Status::Failed(_) => ("Something went wrong...".to_string(), "Check the log below, or open Settings.".to_string()),
        Status::Offline => ("Bocchi is hiding in her closet.".to_string(), "Press Start to bring her out.".to_string()),
    };
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        widgets::app_icon(ui, 84.0);
        ui.add_space(12.0);
        ui.label(RichText::new(title).size(20.0).strong().color(TEXT));
        ui.label(RichText::new(detail).color(MUTED));
    });
}

fn log_panel(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.label(RichText::new("LOG").small().strong().color(MUTED));
    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for line in log::recent() {
            let color = if line.contains("Couldn't") || line.contains("rejected") || line.contains("stopped:") {
                RED
            } else if line.contains("Now playing") {
                PINK
            } else if line.contains("Online as") || line.contains("Updated yt-dlp") {
                GREEN
            } else {
                MUTED
            };
            ui.label(RichText::new(line).monospace().small().color(color));
        }
    });
}
