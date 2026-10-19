//! Bocchi's button icons for Discord. They're drawn here as small pink PNGs and uploaded once
//! as the bot's own "application emojis", so the buttons under messages match the control panel.
//! If uploading fails, the buttons fall back to normal emoji.

use crate::log;
use base64::Engine;
use poise::serenity_prelude::{EmojiId, Http, ReactionType};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::OnceLock;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Bump this when the drawings change, so Discord gets the new versions.
const VERSION: u8 = 1;
const SIZE: u32 = 128;
const PINK: (u8, u8, u8) = (0xF4, 0xA7, 0xBB);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonIcon {
    Back,
    Play,
    Pause,
    Skip,
    Shuffle,
    Loop,
    LoopOne,
    Stop,
    Queue,
    Left,
    Right,
}

impl ButtonIcon {
    const ALL: [ButtonIcon; 11] = [
        Self::Back, Self::Play, Self::Pause, Self::Skip, Self::Shuffle, Self::Loop,
        Self::LoopOne, Self::Stop, Self::Queue, Self::Left, Self::Right,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Back => "back",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Skip => "skip",
            Self::Shuffle => "shuffle",
            Self::Loop => "loop",
            Self::LoopOne => "loopone",
            Self::Stop => "stop",
            Self::Queue => "queue",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The emoji's name on Discord, e.g. `bocchi_play_1`.
    fn emoji_name(self) -> String {
        format!("bocchi_{}_{VERSION}", self.key())
    }

    /// Used if the custom emoji couldn't be uploaded.
    fn fallback(self) -> &'static str {
        match self {
            Self::Back => "⏮️",
            Self::Play => "▶️",
            Self::Pause => "⏸️",
            Self::Skip => "⏭️",
            Self::Shuffle => "🔀",
            Self::Loop => "🔁",
            Self::LoopOne => "🔂",
            Self::Stop => "⏹️",
            Self::Queue => "📃",
            Self::Left => "◀️",
            Self::Right => "▶️",
        }
    }
}

fn loaded() -> &'static OnceLock<HashMap<ButtonIcon, ReactionType>> {
    static LOADED: OnceLock<HashMap<ButtonIcon, ReactionType>> = OnceLock::new();
    &LOADED
}

/// The emoji to put on a button.
pub fn get(icon: ButtonIcon) -> ReactionType {
    loaded()
        .get()
        .and_then(|map| map.get(&icon).cloned())
        .unwrap_or_else(|| ReactionType::Unicode(icon.fallback().to_string()))
}

/// Makes sure every icon exists as an application emoji. Runs once when the bot comes online.
pub async fn setup(http: &Http) {
    let existing = match http.get_application_emojis().await {
        Ok(list) => list,
        Err(e) => {
            log!("Couldn't load button icons, using normal emoji ({e})");
            return;
        }
    };

    let mut map = HashMap::new();
    let mut uploaded = 0;
    for icon in ButtonIcon::ALL {
        let name = icon.emoji_name();
        let id = match existing.iter().find(|e| e.name == name) {
            Some(emoji) => Some(emoji.id),
            None => upload(http, icon, &name).await,
        };
        if let Some(id) = id {
            if !existing.iter().any(|e| e.id == id) {
                uploaded += 1;
            }
            map.insert(icon, ReactionType::Custom { animated: false, id, name: Some(name) });
        }
    }
    if uploaded > 0 {
        log!("Uploaded {uploaded} button icons to Discord");
    }
    let _ = loaded().set(map);
}

async fn upload(http: &Http, icon: ButtonIcon, name: &str) -> Option<EmojiId> {
    let png = draw(icon)?;
    let image = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png));
    let body = serde_json::json!({ "name": name, "image": image });
    match http.create_application_emoji(&body).await {
        Ok(emoji) => Some(emoji.id),
        Err(e) => {
            log!("Couldn't upload the {name} icon: {e}");
            None
        }
    }
}

// ============================== Drawing ==============================

/// A 128×128 canvas. Shapes are given on a 0-1 grid, like the control panel's icons.
struct Canvas {
    pixmap: Pixmap,
    paint: Paint<'static>,
}

impl Canvas {
    fn new() -> Option<Self> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(PINK.0, PINK.1, PINK.2, 255);
        paint.anti_alias = true;
        Some(Self { pixmap: Pixmap::new(SIZE, SIZE)?, paint })
    }

    fn at(x: f32, y: f32) -> (f32, f32) {
        const MARGIN: f32 = 10.0;
        let scale = SIZE as f32 - 2.0 * MARGIN;
        (MARGIN + x * scale, MARGIN + y * scale)
    }

    /// A filled shape with rounded corners (the outline is stroked with round joins).
    fn shape(&mut self, points: &[(f32, f32)], roundness: f32) {
        let Some(path) = path(points, true) else { return };
        self.pixmap.fill_path(&path, &self.paint, FillRule::Winding, Transform::identity(), None);
        let stroke = Stroke { width: roundness, line_join: LineJoin::Round, ..Default::default() };
        self.pixmap.stroke_path(&path, &self.paint, &stroke, Transform::identity(), None);
    }

    /// An open line with round ends.
    fn line(&mut self, points: &[(f32, f32)], width: f32) {
        let Some(path) = path(points, false) else { return };
        let stroke = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
        self.pixmap.stroke_path(&path, &self.paint, &stroke, Transform::identity(), None);
    }

    fn bar(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.shape(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], 10.0);
    }

    fn arrow(&mut self, tip: (f32, f32), direction: (f32, f32), size: f32) {
        let back = (tip.0 - direction.0 * size, tip.1 - direction.1 * size);
        let side = (-direction.1 * size * 0.65, direction.0 * size * 0.65);
        self.shape(&[tip, (back.0 + side.0, back.1 + side.1), (back.0 - side.0, back.1 - side.1)], 6.0);
    }
}

fn path(points: &[(f32, f32)], closed: bool) -> Option<tiny_skia::Path> {
    let mut builder = PathBuilder::new();
    let (x, y) = Canvas::at(points[0].0, points[0].1);
    builder.move_to(x, y);
    for &(px, py) in &points[1..] {
        let (x, y) = Canvas::at(px, py);
        builder.line_to(x, y);
    }
    if closed {
        builder.close();
    }
    builder.finish()
}

/// Points on a circle; angles are in turns (0 = right, 0.25 = down).
fn arc(cx: f32, cy: f32, radius: f32, start: f32, end: f32) -> Vec<(f32, f32)> {
    (0..=32)
        .map(|i| {
            let t = (start + (end - start) * i as f32 / 32.0) * TAU;
            (cx + radius * t.cos(), cy + radius * t.sin())
        })
        .collect()
}

fn draw(icon: ButtonIcon) -> Option<Vec<u8>> {
    let mut c = Canvas::new()?;
    const LINE: f32 = 13.0;
    match icon {
        ButtonIcon::Play => c.shape(&[(0.28, 0.14), (0.88, 0.5), (0.28, 0.86)], 14.0),
        ButtonIcon::Pause => {
            c.bar(0.24, 0.14, 0.42, 0.86);
            c.bar(0.58, 0.14, 0.76, 0.86);
        }
        ButtonIcon::Skip => {
            c.shape(&[(0.14, 0.16), (0.68, 0.5), (0.14, 0.84)], 12.0);
            c.bar(0.72, 0.16, 0.86, 0.84);
        }
        ButtonIcon::Back => {
            c.shape(&[(0.86, 0.16), (0.86, 0.84), (0.32, 0.5)], 12.0);
            c.bar(0.14, 0.16, 0.28, 0.84);
        }
        ButtonIcon::Stop => c.bar(0.2, 0.2, 0.8, 0.8),
        ButtonIcon::Shuffle => {
            c.line(&[(0.06, 0.28), (0.3, 0.28), (0.64, 0.72), (0.8, 0.72)], LINE);
            c.line(&[(0.06, 0.72), (0.3, 0.72), (0.64, 0.28), (0.8, 0.28)], LINE);
            c.arrow((0.96, 0.28), (1.0, 0.0), 0.2);
            c.arrow((0.96, 0.72), (1.0, 0.0), 0.2);
        }
        ButtonIcon::Loop | ButtonIcon::LoopOne => {
            c.line(&arc(0.5, 0.5, 0.36, 0.1, 0.9), LINE);
            let end = arc(0.5, 0.5, 0.36, 0.9, 0.9)[0];
            c.arrow((end.0 + 0.07, end.1 - 0.03), (0.5, -0.87), 0.22);
            if icon == ButtonIcon::LoopOne {
                c.bar(0.45, 0.34, 0.55, 0.66);
            }
        }
        ButtonIcon::Queue => {
            c.line(&[(0.12, 0.24), (0.88, 0.24)], LINE);
            c.line(&[(0.12, 0.5), (0.88, 0.5)], LINE);
            c.line(&[(0.12, 0.76), (0.6, 0.76)], LINE);
        }
        ButtonIcon::Left => c.line(&[(0.66, 0.16), (0.32, 0.5), (0.66, 0.84)], LINE + 2.0),
        ButtonIcon::Right => c.line(&[(0.34, 0.16), (0.68, 0.5), (0.34, 0.84)], LINE + 2.0),
    }
    c.pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws() {
        for icon in ButtonIcon::ALL {
            let png = draw(icon).expect("icon draws");
            assert!(png.len() > 100 && png.len() < 256 * 1024, "Discord's emoji size limit");
            let name = icon.emoji_name();
            assert!(name.len() <= 32 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            // Set BOCCHI_ICON_DIR to save the icons and look at them.
            if let Ok(dir) = std::env::var("BOCCHI_ICON_DIR") {
                std::fs::write(format!("{dir}/{}.png", icon.key()), &png).unwrap();
            }
        }
    }
}
