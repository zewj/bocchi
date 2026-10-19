//! Bocchi's look: her pink, plus the yellow and blue of her cube hair clips.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, IconData, Stroke, Visuals};
use std::sync::Arc;

pub const PINK: Color32 = Color32::from_rgb(0xF4, 0xA7, 0xBB);
pub const PINK_DEEP: Color32 = Color32::from_rgb(0xD9, 0x5F, 0x8C);
pub const YELLOW: Color32 = Color32::from_rgb(0xF7, 0xD4, 0x4C);
pub const BLUE: Color32 = Color32::from_rgb(0x5B, 0xA4, 0xE6);
pub const GREEN: Color32 = Color32::from_rgb(0x7B, 0xD8, 0x8F);
pub const RED: Color32 = Color32::from_rgb(0xED, 0x5A, 0x6A);
pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0x8F, 0xA0);
/// Main text color.
pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xE6, 0xF1);
/// Dark text/icons on pink.
pub const INK: Color32 = Color32::from_rgb(0x2A, 0x15, 0x22);

const BACKGROUND: Color32 = Color32::from_rgb(0x14, 0x11, 0x17);
const PANEL: Color32 = Color32::from_rgb(0x1B, 0x17, 0x1F);
pub const CARD: Color32 = Color32::from_rgb(0x25, 0x1F, 0x2A);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x33, 0x2A, 0x38);

/// The app icon picture (also shown next to the name in the window). build.rs picks your own
/// `assets/icon.png` if it exists, otherwise the default one.
pub const ICON_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/app_icon.png"));

/// RocknRoll One: a free rock-and-roll style display font, used for the "Bocchi" title.
/// Trimmed to basic Latin letters (67 KB instead of 2.6 MB with all the Japanese glyphs).
const TITLE_FONT: &[u8] = include_bytes!("../assets/RocknRollOne-Latin.ttf");

/// The font family for titles, e.g. `RichText::new("Bocchi").family(theme::title_font())`.
pub fn title_font() -> FontFamily {
    FontFamily::Name("title".into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("rocknroll".into(), Arc::new(FontData::from_static(TITLE_FONT)));
    fonts.families.insert(title_font(), vec!["rocknroll".into()]);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = CARD;
    visuals.hyperlink_color = PINK;
    visuals.selection.bg_fill = PINK_DEEP.gamma_multiply(0.6);
    visuals.selection.stroke = Stroke::new(1.0, PINK);
    visuals.slider_trailing_fill = true;
    visuals.window_corner_radius = CornerRadius::same(12);

    let rounded = CornerRadius::same(8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = rounded;
    }
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, PINK.gamma_multiply(0.6));
    visuals.widgets.active.weak_bg_fill = PINK_DEEP;
    visuals.widgets.active.bg_fill = PINK_DEEP;
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
    });
}

/// The window icon (see `ICON_PNG`), built into the program.
/// Falls back to the drawn hair-clip tile if the picture can't be read.
pub fn icon() -> IconData {
    match image::load_from_memory(ICON_PNG) {
        Ok(picture) => {
            let picture = picture.to_rgba8();
            let (width, height) = picture.dimensions();
            IconData { rgba: picture.into_raw(), width, height }
        }
        Err(_) => drawn_icon(),
    }
}

/// A pink tile with Bocchi's yellow and blue cube hair clips, drawn in code.
fn drawn_icon() -> IconData {
    const SIZE: u32 = 64;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let mut paint = |x: u32, y: u32, color: Color32| {
        let i = ((y * SIZE + x) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&color.to_array());
    };

    for y in 0..SIZE {
        for x in 0..SIZE {
            // Rounded-corner pink tile.
            let (dx, dy) = (x.min(SIZE - 1 - x), y.min(SIZE - 1 - y));
            let corner = 12;
            let outside = dx < corner && dy < corner && {
                let (cx, cy) = (corner - dx, corner - dy);
                cx * cx + cy * cy > corner * corner
            };
            if !outside {
                paint(x, y, PINK);
            }
        }
    }
    // Two cube hair clips, each with a darker edge so they read as cubes.
    let mut cube = |left: u32, top: u32, face: Color32| {
        for y in top..top + 18 {
            for x in left..left + 18 {
                let edge = x >= left + 14 || y >= top + 14;
                paint(x, y, if edge { face.gamma_multiply(0.7) } else { face });
            }
        }
    };
    cube(12, 14, YELLOW);
    cube(34, 30, BLUE);

    IconData { rgba, width: SIZE, height: SIZE }
}
