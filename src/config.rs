//! Bot settings, stored in `.env` next to the program.

use std::path::PathBuf;

const FILE: &str = ".env";

/// Where `.env` lives: next to the exe if there's one there, else in the current folder
/// (e.g. the project folder during `cargo run`). New files go next to the exe.
fn path() -> PathBuf {
    let next_to_exe = crate::setup::exe_dir().join(FILE);
    let current_folder = PathBuf::from(FILE);
    if !next_to_exe.exists() && current_folder.exists() { current_folder } else { next_to_exe }
}

#[derive(Clone, Default)]
pub struct Config {
    pub token: String,
    pub prefix: String,
    /// Only kept to clean up slash commands that older versions registered in one server.
    pub guild_id: String,
    /// Start the bot as soon as the control panel opens.
    pub autostart: bool,
}

impl Config {
    pub fn load() -> Self {
        let mut config = Config { prefix: "!".into(), autostart: true, ..Default::default() };
        if let Ok(values) = dotenvy::from_path_iter(path()) {
            for (key, value) in values.flatten() {
                match key.as_str() {
                    "DISCORD_TOKEN" => config.token = value,
                    "PREFIX" => config.prefix = value,
                    "GUILD_ID" => config.guild_id = value,
                    "AUTOSTART" => config.autostart = value.trim() != "false",
                    _ => {}
                }
            }
        }
        // Values set outside the file (e.g. by a hosting service) win.
        if let Ok(token) = std::env::var("DISCORD_TOKEN") {
            config.token = token;
        }
        config
    }

    pub fn save(&self) -> std::io::Result<()> {
        let mut lines = vec![
            format!("DISCORD_TOKEN={}", self.token.trim()),
            format!("PREFIX={}", self.prefix.trim()),
            "# Start the bot when the control panel opens".to_string(),
            format!("AUTOSTART={}", self.autostart),
        ];
        if !self.guild_id.trim().is_empty() {
            lines.push(format!("GUILD_ID={}", self.guild_id.trim()));
        }
        std::fs::write(path(), lines.join("\n") + "\n")
    }

    pub fn has_token(&self) -> bool {
        let token = self.token.trim();
        !token.is_empty() && !token.starts_with("paste-")
    }
}
