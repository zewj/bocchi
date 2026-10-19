//! The control panel window: start/stop the bot, settings, every server it's playing in,
//! playback controls, the queue, and the log.

use crate::bot::{self, Bot, Status, StatusLock};
use crate::config::Config;
use crate::control::{self, Action, PlayerView};
use crate::data::{Error, Guilds, LoopMode};
use crate::{log, util};
use eframe::egui::{self, Color32, RichText};
use poise::serenity_prelude::GuildId;
use poise::ChoiceParameter;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::runtime::Handle;

const ACCENT: Color32 = Color32::from_rgb(0x9B, 0x59, 0xB6);
const GREEN: Color32 = Color32::from_rgb(0x57, 0xF2, 0x87);
const RED: Color32 = Color32::from_rgb(0xED, 0x42, 0x45);
const YELLOW: Color32 = Color32::from_rgb(0xFE, 0xE7, 0x5C);
const REFRESH: Duration = Duration::from_millis(500);

pub fn run(runtime: Handle) -> Result<(), Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bocchi")
            .with_inner_size([980.0, 680.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Bocchi",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
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
        if panel.config.has_token() {
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

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, &status));
        egui::Panel::bottom("log").resizable(true).default_size(160.0).show(ui, log_panel);
        egui::Panel::left("servers").resizable(true).default_size(220.0).show(ui, |ui| {
            self.server_list(ui, &players, &status);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            match players.iter().find(|p| Some(p.guild) == self.selected) {
                Some(player) => self.player(ui, player),
                None => empty_state(ui, &status),
            }
        });

        let ctx = ui.ctx().clone();
        self.settings_window(&ctx);
    }
}

// ============================== Sections ==============================

impl ControlPanel {
    fn top_bar(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("🎵 Bocchi").size(20.0).strong().color(ACCENT));
            ui.add_space(12.0);

            let (color, text) = match status {
                Status::Offline => (Color32::GRAY, "Offline".to_string()),
                Status::Starting => (YELLOW, "Starting...".to_string()),
                Status::Online { name, since } => {
                    (GREEN, format!("Online as {name} · up {}", util::format_time(since.elapsed())))
                }
                Status::Failed(reason) => (RED, format!("Stopped: {reason}")),
            };
            ui.label(RichText::new("●").color(color).size(16.0));
            ui.label(text);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙ Settings").clicked() {
                    self.settings_open = !self.settings_open;
                }
                let running = matches!(status, Status::Starting | Status::Online { .. });
                if running {
                    if ui.button(RichText::new("⏹ Stop bot").color(RED)).clicked() {
                        self.stop_bot();
                    }
                } else if ui.button(RichText::new("▶ Start bot").color(GREEN)).clicked() {
                    self.start_bot();
                }
            });
        });
        ui.add_space(6.0);
    }

    fn server_list(&mut self, ui: &mut egui::Ui, players: &[PlayerView], status: &Status) {
        ui.add_space(6.0);
        ui.heading("Servers");
        ui.separator();
        if players.is_empty() {
            let hint = if matches!(status, Status::Online { .. }) {
                "Not in any voice channel.\nUse !play in Discord to start."
            } else {
                "Start the bot to see servers."
            };
            ui.label(RichText::new(hint).weak());
        }
        for player in players {
            let icon = match (&player.now, player.paused) {
                (None, _) => "💤",
                (Some(_), true) => "⏸",
                (Some(_), false) => "🔊",
            };
            let text = format!("{icon} {}\n    #{}", player.server, player.channel);
            if ui.selectable_label(self.selected == Some(player.guild), text).clicked() {
                self.selected = Some(player.guild);
            }
        }
    }

    fn player(&mut self, ui: &mut egui::Ui, player: &PlayerView) {
        let guild = player.guild;
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} · 🔊 {}", player.server, player.channel)).weak());
        ui.add_space(8.0);

        // Now playing
        match &player.now {
            Some((song, position)) => {
                ui.hyperlink_to(RichText::new(&song.title).size(18.0).strong(), &song.url);
                let fraction = song
                    .duration
                    .filter(|d| !d.is_zero())
                    .map(|d| (position.as_secs_f32() / d.as_secs_f32()).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                let times = format!("{} / {}", util::format_time(*position), util::format_length(song.duration));
                ui.add(egui::ProgressBar::new(fraction).fill(ACCENT).text(times));
            }
            None => {
                ui.label(RichText::new("Nothing playing").size(18.0).weak());
            }
        }
        ui.add_space(8.0);

        // Controls
        ui.horizontal(|ui| {
            let playing = player.now.is_some();
            if player.paused {
                if ui.add_enabled(playing, egui::Button::new("▶ Resume")).clicked() {
                    self.act(guild, Action::Resume);
                }
            } else if ui.add_enabled(playing, egui::Button::new("⏸ Pause")).clicked() {
                self.act(guild, Action::Pause);
            }
            if ui.add_enabled(playing, egui::Button::new("⏭ Skip")).clicked() {
                self.act(guild, Action::Skip);
            }
            if ui.add_enabled(playing, egui::Button::new("⏮ Replay")).clicked() {
                self.act(guild, Action::Replay);
            }
            if ui.add_enabled(player.upcoming.len() > 1, egui::Button::new("🔀 Shuffle")).clicked() {
                self.act(guild, Action::Shuffle);
            }
            if ui.add_enabled(playing, egui::Button::new("⏹ Stop")).clicked() {
                self.act(guild, Action::Stop);
            }
            if ui.button(RichText::new("👋 Leave").color(RED)).clicked() {
                self.act(guild, Action::Leave);
            }
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Volume");
            let mut volume = self.volume_drag.unwrap_or(player.volume * 100.0);
            let slider = ui.add(egui::Slider::new(&mut volume, 0.0..=200.0).suffix("%").integer());
            self.volume_drag = slider.dragged().then_some(volume);
            if slider.changed() {
                self.act(guild, Action::Volume(volume / 100.0));
                // Show the new value right away instead of waiting for the next refresh.
                if let Some(view) = self.players.lock().unwrap().iter_mut().find(|p| p.guild == guild) {
                    view.volume = volume / 100.0;
                }
            }

            ui.add_space(16.0);
            ui.label("Loop");
            let mut mode = player.loop_mode;
            egui::ComboBox::from_id_salt("loop").selected_text(mode.name()).show_ui(ui, |ui| {
                for option in [LoopMode::Off, LoopMode::Song, LoopMode::Queue] {
                    ui.selectable_value(&mut mode, option, option.name());
                }
            });
            if mode != player.loop_mode {
                self.act(guild, Action::Loop(mode));
            }
        });
        ui.add_space(8.0);

        // Add a song
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.add_query)
                    .hint_text("Song name or link")
                    .desired_width(ui.available_width() - 80.0),
            );
            let pressed_enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("➕ Add").clicked() || pressed_enter) && !self.add_query.trim().is_empty() {
                self.act(guild, Action::Add(self.add_query.trim().to_string()));
                self.add_query.clear();
            }
        });
        ui.add_space(8.0);
        ui.separator();

        // Queue
        let total: Duration = player.upcoming.iter().filter_map(|s| s.duration).sum();
        ui.label(RichText::new(format!("Up next · {} songs · {}", player.upcoming.len(), util::format_time(total))).strong());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if player.upcoming.is_empty() {
                ui.label(RichText::new("The queue is empty.").weak());
            }
            for (i, song) in player.upcoming.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.small_button("✖").on_hover_text("Remove").clicked() {
                        // +1 because position 0 is the song playing now.
                        self.act(guild, Action::Remove(i + 1));
                    }
                    ui.label(RichText::new(format!("{:>3}.", i + 1)).monospace().weak());
                    ui.label(&song.title);
                    ui.label(RichText::new(util::format_length(song.duration)).weak());
                });
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("⚙ Settings").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
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
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.guild_id)
                        .hint_text("optional")
                        .desired_width(200.0),
                );
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("Get the token in the developer portal → your app → Bot → Reset Token.\nAlso turn on Message Content Intent there.")
                    .weak()
                    .small(),
            );
            ui.add_space(6.0);
            if ui.button("💾 Save").clicked() {
                match self.config.save() {
                    Ok(()) => log!("Settings saved. Restart the bot to use them."),
                    Err(e) => log!("Couldn't save settings: {e}"),
                }
            }
        });
        self.settings_open = open;
    }
}

fn empty_state(ui: &mut egui::Ui, status: &Status) {
    ui.centered_and_justified(|ui| {
        let text = match status {
            Status::Online { .. } => "Join a voice channel and use !play in Discord.\nIt'll show up here.",
            Status::Starting => "Connecting to Discord...",
            _ => "Press ▶ Start bot to go online.",
        };
        ui.label(RichText::new(text).size(16.0).weak());
    });
}

fn log_panel(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.label(RichText::new("Log").strong());
    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
        for line in log::recent() {
            ui.label(RichText::new(line).monospace().small());
        }
    });
}
