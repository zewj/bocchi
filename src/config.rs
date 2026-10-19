//! Bot settings, stored in `.env` next to the program.

use std::path::Path;

const FILE: &str = ".env";

#[derive(Clone, Default)]
pub struct Config {
    pub token: String,
    pub prefix: String,
    /// Optional test server ID: slash commands show up there instantly.
    pub guild_id: String,
}

impl Config {
    pub fn load() -> Self {
        let mut config = Config { prefix: "!".into(), ..Default::default() };
        if let Ok(values) = dotenvy::from_path_iter(Path::new(FILE)) {
            for (key, value) in values.flatten() {
                match key.as_str() {
                    "DISCORD_TOKEN" => config.token = value,
                    "PREFIX" => config.prefix = value,
                    "GUILD_ID" => config.guild_id = value,
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
        let text = format!(
            "DISCORD_TOKEN={}\nPREFIX={}\n# Optional: your test server ID so slash commands show up instantly\nGUILD_ID={}\n",
            self.token.trim(),
            self.prefix.trim(),
            self.guild_id.trim()
        );
        std::fs::write(FILE, text)
    }

    pub fn has_token(&self) -> bool {
        let token = self.token.trim();
        !token.is_empty() && !token.starts_with("paste-")
    }
}
