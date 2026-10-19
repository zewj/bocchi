//! Bocchi's own widgets: icons drawn with code (no emoji, no image files) and custom buttons
//! with hover and press animations.

use crate::theme::{CARD, CARD_HOVER, INK, MUTED, PINK, PINK_DEEP, RED, TEXT};
use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, Widget,
};
use std::f32::consts::TAU;

// ============================== Icons ==============================

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    Shuffle,
    Loop,
    LoopOne,
    Leave,
    Plus,
    Close,
    Up,
    Down,
    Gear,
    Volume,
    Mute,
    Note,
    Power,
}

/// Draws an icon inside `rect`. Shapes are described on a 0-1 grid and scaled to fit.
pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let side = rect.width().min(rect.height());
    let origin = rect.center() - Vec2::splat(side / 2.0);
    let p = |x: f32, y: f32| origin + Vec2::new(x * side, y * side);
    let line = Stroke::new((side * 0.1).max(1.5), color);
    let fill = |points: Vec<Pos2>| Shape::convex_polygon(points, color, Stroke::NONE);
    // Triangles with softly rounded corners, like Spotify's controls.
    let soft = |points: Vec<Pos2>| Shape::convex_polygon(rounded(&points, side * 0.07), color, Stroke::NONE);
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Shape::rect_filled(Rect::from_min_max(p(x0, y0), p(x1, y1)), CornerRadius::same((side * 0.07) as u8), color)
    };

    match icon {
        Icon::Play => {
            painter.add(soft(vec![p(0.3, 0.17), p(0.85, 0.5), p(0.3, 0.83)]));
        }
        Icon::Pause => {
            painter.add(bar(0.27, 0.18, 0.42, 0.82));
            painter.add(bar(0.58, 0.18, 0.73, 0.82));
        }
        Icon::Stop => {
            painter.add(bar(0.24, 0.24, 0.76, 0.76));
        }
        Icon::Next => {
            painter.add(soft(vec![p(0.18, 0.18), p(0.68, 0.5), p(0.18, 0.82)]));
            painter.add(bar(0.7, 0.18, 0.82, 0.82));
        }
        Icon::Previous => {
            painter.add(soft(vec![p(0.82, 0.18), p(0.82, 0.82), p(0.32, 0.5)]));
            painter.add(bar(0.18, 0.18, 0.3, 0.82));
        }
        Icon::Shuffle => {
            painter.add(Shape::line(vec![p(0.12, 0.3), p(0.32, 0.3), p(0.62, 0.7), p(0.8, 0.7)], line));
            painter.add(Shape::line(vec![p(0.12, 0.7), p(0.32, 0.7), p(0.62, 0.3), p(0.8, 0.3)], line));
            arrow_head(painter, p(0.9, 0.3), Vec2::X, side * 0.2, color);
            arrow_head(painter, p(0.9, 0.7), Vec2::X, side * 0.2, color);
        }
        Icon::Loop | Icon::LoopOne => {
            let center = p(0.5, 0.5);
            painter.add(Shape::line(arc(center, side * 0.3, 0.12, 0.92), line));
            let end = arc(center, side * 0.3, 0.92, 0.92)[0];
            arrow_head(painter, end + Vec2::new(side * 0.06, -side * 0.02), Vec2::new(0.6, -1.0).normalized(), side * 0.2, color);
            if icon == Icon::LoopOne {
                painter.add(bar(0.45, 0.36, 0.55, 0.64));
            }
        }
        Icon::Leave => {
            painter.add(Shape::line(vec![p(0.5, 0.2), p(0.2, 0.2), p(0.2, 0.8), p(0.5, 0.8)], line));
            painter.line_segment([p(0.38, 0.5), p(0.78, 0.5)], line);
            arrow_head(painter, p(0.88, 0.5), Vec2::X, side * 0.22, color);
        }
        Icon::Plus => {
            painter.line_segment([p(0.5, 0.2), p(0.5, 0.8)], line);
            painter.line_segment([p(0.2, 0.5), p(0.8, 0.5)], line);
        }
        Icon::Close => {
            painter.line_segment([p(0.25, 0.25), p(0.75, 0.75)], line);
            painter.line_segment([p(0.75, 0.25), p(0.25, 0.75)], line);
        }
        Icon::Up => {
            painter.add(Shape::line(vec![p(0.22, 0.64), p(0.5, 0.36), p(0.78, 0.64)], line));
        }
        Icon::Down => {
            painter.add(Shape::line(vec![p(0.22, 0.36), p(0.5, 0.64), p(0.78, 0.36)], line));
        }
        Icon::Gear => {
            let center = p(0.5, 0.5);
            for i in 0..8 {
                let angle = i as f32 / 8.0 * TAU;
                let dir = Vec2::angled(angle);
                painter.line_segment([center + dir * side * 0.26, center + dir * side * 0.42], Stroke::new(side * 0.14, color));
            }
            painter.circle_stroke(center, side * 0.24, line);
            painter.circle_filled(center, side * 0.08, color);
        }
        Icon::Volume | Icon::Mute => {
            painter.add(fill(vec![p(0.12, 0.38), p(0.3, 0.38), p(0.5, 0.2), p(0.5, 0.8), p(0.3, 0.62), p(0.12, 0.62)]));
            if icon == Icon::Mute {
                painter.line_segment([p(0.62, 0.38), p(0.86, 0.62)], line);
                painter.line_segment([p(0.86, 0.38), p(0.62, 0.62)], line);
            } else {
                let center = p(0.5, 0.5);
                painter.add(Shape::line(arc(center, side * 0.18, -0.11, 0.11), line));
                painter.add(Shape::line(arc(center, side * 0.34, -0.12, 0.12), line));
            }
        }
        Icon::Note => {
            painter.circle_filled(p(0.36, 0.72), side * 0.13, color);
            painter.line_segment([p(0.48, 0.72), p(0.48, 0.2)], line);
            painter.add(Shape::line(vec![p(0.48, 0.2), p(0.76, 0.3), p(0.76, 0.42)], line));
        }
        Icon::Power => {
            let center = p(0.5, 0.54);
            painter.add(Shape::line(arc(center, side * 0.3, -0.18, 0.68), line));
            painter.line_segment([p(0.5, 0.14), p(0.5, 0.5)], line);
        }
    }
}

/// Points along a circle. Angles are in turns (0 = right, 0.25 = down), going clockwise.
fn arc(center: Pos2, radius: f32, start: f32, end: f32) -> Vec<Pos2> {
    let steps = 24;
    (0..=steps)
        .map(|i| {
            let t = start + (end - start) * i as f32 / steps as f32;
            center + Vec2::angled(t * TAU) * radius
        })
        .collect()
}

/// Rounds the corners of a convex polygon by replacing each corner with a small arc.
fn rounded(points: &[Pos2], radius: f32) -> Vec<Pos2> {
    let n = points.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (prev, corner, next) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let to_prev = (prev - corner).normalized();
        let to_next = (next - corner).normalized();
        let half_angle = to_prev.dot(to_next).clamp(-1.0, 1.0).acos() / 2.0;
        // How far along each edge the curve starts, and where its center sits.
        let cut = radius / half_angle.tan();
        let center = corner + (to_prev + to_next).normalized() * (radius / half_angle.sin());
        let start = corner + to_prev * cut;
        let end = corner + to_next * cut;
        let (a0, a1) = ((start - center).angle(), (end - center).angle());
        let mut sweep = a1 - a0;
        if sweep > std::f32::consts::PI {
            sweep -= TAU;
        } else if sweep < -std::f32::consts::PI {
            sweep += TAU;
        }
        for step in 0..=6 {
            let angle = a0 + sweep * step as f32 / 6.0;
            out.push(center + Vec2::angled(angle) * radius);
        }
    }
    out
}

fn arrow_head(painter: &Painter, tip: Pos2, direction: Vec2, size: f32, color: Color32) {
    let back = tip - direction * size;
    let side = direction.rot90() * size * 0.6;
    painter.add(Shape::convex_polygon(vec![tip, back + side, back - side], color, Stroke::NONE));
}

// ============================== Buttons ==============================

#[derive(Clone, Copy, PartialEq)]
enum Style {
    /// Pink, for the main action.
    Primary,
    /// Subtle, for everything else.
    Ghost,
    /// Red, for leaving/stopping.
    Danger,
    /// No background at all, just the icon (Spotify's back/forward/shuffle/repeat).
    Plain,
    /// A white circle with a dark icon (Spotify's play/pause).
    Light,
}

/// A rounded button with a drawn icon and optional text.
pub struct IconButton<'a> {
    icon: Icon,
    text: Option<&'a str>,
    style: Style,
    active: bool,
    icon_size: f32,
    round: bool,
    enabled: bool,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Icon) -> Self {
        Self { icon, text: None, style: Style::Ghost, active: false, icon_size: 16.0, round: false, enabled: true }
    }
    pub fn text(mut self, text: &'a str) -> Self {
        self.text = Some(text);
        self
    }
    pub fn primary(mut self) -> Self {
        self.style = Style::Primary;
        self
    }
    /// Icon only, no background. Gets brighter and a little bigger on hover.
    pub fn plain(mut self) -> Self {
        self.style = Style::Plain;
        self
    }
    /// A white circle with a dark icon, like Spotify's play button. Use with `round()`.
    pub fn light(mut self) -> Self {
        self.style = Style::Light;
        self
    }
    pub fn danger(mut self) -> Self {
        self.style = Style::Danger;
        self
    }
    /// Lights the icon up in pink, for toggles like loop.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    pub fn size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }
    /// A circle instead of a rounded rectangle.
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let font = FontId::proportional(14.0);
        let padding = match self.style {
            Style::Plain => Vec2::splat(6.0),
            _ => Vec2::new(if self.round { 0.0 } else { 10.0 }, 7.0),
        };
        let galley = self.text.map(|t| ui.painter().layout_no_wrap(t.to_string(), font, TEXT));

        let mut size = Vec2::splat(self.icon_size) + padding * 2.0;
        if let Some(galley) = &galley {
            size.x += 7.0 + galley.size().x;
        }
        if self.round {
            size = Vec2::splat(self.icon_size * 2.4);
        }
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        if !ui.is_rect_visible(rect) {
            return response;
        }

        let hover = ui.ctx().animate_bool(response.id.with("hover"), self.enabled && response.hovered());
        let pressed = self.enabled && response.is_pointer_button_down_on();
        let full_rect = rect;
        // Spotify-style buttons grow a little on hover; everything shrinks a little when pressed.
        let grows = matches!(self.style, Style::Plain | Style::Light);
        let scale = if pressed { 0.94 } else if grows { 1.0 + 0.06 * hover } else { 1.0 };
        let rect = Rect::from_center_size(rect.center(), rect.size() * scale);
        let icon_size = self.icon_size * scale;

        let (base, lit, icon_color) = match self.style {
            Style::Primary => (PINK, PINK.lerp_to_gamma(Color32::WHITE, 0.25), INK),
            Style::Danger => (RED.gamma_multiply(0.18), RED.gamma_multiply(0.36), RED.lerp_to_gamma(Color32::WHITE, 0.3)),
            Style::Ghost => (CARD, CARD_HOVER, if self.active { PINK } else { TEXT }),
            Style::Light => (Color32::from_rgb(0xF2, 0xF0, 0xF3), Color32::WHITE, Color32::BLACK),
            Style::Plain => {
                let icon = if self.active { PINK } else { MUTED.lerp_to_gamma(Color32::WHITE, hover) };
                (Color32::TRANSPARENT, Color32::TRANSPARENT, icon)
            }
        };
        let mut background = base.lerp_to_gamma(lit, hover);
        let mut icon_color = icon_color;
        if !self.enabled {
            background = background.gamma_multiply(0.5);
            icon_color = icon_color.gamma_multiply(0.35);
        }

        let radius = if self.round { CornerRadius::same(255) } else { CornerRadius::same(9) };
        let painter = ui.painter();
        painter.rect_filled(rect, radius, background);
        if self.style == Style::Ghost && (hover > 0.0 || self.active) {
            let glow = PINK.gamma_multiply(if self.active { 0.7 } else { 0.5 * hover });
            painter.rect_stroke(rect, radius, Stroke::new(1.0, glow), StrokeKind::Inside);
        }

        let icon_rect = if self.round || self.text.is_none() {
            Rect::from_center_size(rect.center(), Vec2::splat(icon_size))
        } else {
            Rect::from_min_size(
                Pos2::new(rect.left() + padding.x, rect.center().y - icon_size / 2.0),
                Vec2::splat(icon_size),
            )
        };
        paint_icon(painter, icon_rect, self.icon, icon_color);
        // Spotify marks an active shuffle/repeat with a small dot underneath.
        if self.style == Style::Plain && self.active {
            painter.circle_filled(Pos2::new(full_rect.center().x, full_rect.bottom() + 1.0), 2.0, PINK);
        }
        if let Some(galley) = galley {
            let at = Pos2::new(icon_rect.right() + 7.0, rect.center().y - galley.size().y / 2.0);
            painter.galley(at, galley, icon_color);
        }

        if self.enabled {
            response.on_hover_cursor(CursorIcon::PointingHand)
        } else {
            response
        }
    }
}

// ============================== Small decorations ==============================

/// The app icon as a round picture, e.g. next to the name in the top bar.
pub fn app_icon(ui: &mut Ui, size: f32) {
    let picture = egui::Image::from_bytes("bytes://bocchi-icon.png", crate::theme::ICON_PNG)
        .fit_to_exact_size(Vec2::splat(size))
        .corner_radius((size / 2.0) as u8);
    ui.add(picture);
}

/// A small colored status dot, sized to sit next to text.
pub fn dot(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

/// A large icon on a tinted tile, used where there's no album art.
pub fn placeholder(ui: &mut Ui, size: Vec2, icon: Icon) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(8), PINK_DEEP.gamma_multiply(0.25));
    let icon_size = size.y * 0.45;
    paint_icon(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(icon_size)), icon, PINK);
}

/// Just an icon, no button (e.g. next to the volume slider).
pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon(ui.painter(), rect, icon, color);
    response
}

/// Lets callers write `ui.add(button(Icon::Play))`.
pub fn button<'a>(icon: Icon) -> IconButton<'a> {
    IconButton::new(icon)
}
