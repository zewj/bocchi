//! Bocchi's own widgets: icons drawn with code (no emoji, no image files) and custom buttons
//! with hover and press animations.

use crate::theme::{CARD, CARD_HOVER, INK, PINK, PINK_DEEP, RED, TEXT, YELLOW, BLUE};
use eframe::egui::{
    Color32, CornerRadius, CursorIcon, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
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
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Shape::rect_filled(Rect::from_min_max(p(x0, y0), p(x1, y1)), CornerRadius::same((side * 0.06) as u8), color)
    };

    match icon {
        Icon::Play => {
            painter.add(fill(vec![p(0.3, 0.18), p(0.82, 0.5), p(0.3, 0.82)]));
        }
        Icon::Pause => {
            painter.add(bar(0.26, 0.2, 0.42, 0.8));
            painter.add(bar(0.58, 0.2, 0.74, 0.8));
        }
        Icon::Stop => {
            painter.add(bar(0.24, 0.24, 0.76, 0.76));
        }
        Icon::Next => {
            painter.add(fill(vec![p(0.2, 0.22), p(0.62, 0.5), p(0.2, 0.78)]));
            painter.add(bar(0.66, 0.22, 0.8, 0.78));
        }
        Icon::Previous => {
            painter.add(fill(vec![p(0.8, 0.22), p(0.8, 0.78), p(0.38, 0.5)]));
            painter.add(bar(0.2, 0.22, 0.34, 0.78));
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
        let padding = Vec2::new(if self.round { 0.0 } else { 10.0 }, 7.0);
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
        let rect = if pressed { rect.shrink(1.5) } else { rect };

        let (base, lit, icon_color) = match self.style {
            Style::Primary => (PINK, PINK.lerp_to_gamma(Color32::WHITE, 0.25), INK),
            Style::Danger => (RED.gamma_multiply(0.18), RED.gamma_multiply(0.36), RED.lerp_to_gamma(Color32::WHITE, 0.3)),
            Style::Ghost => (CARD, CARD_HOVER, if self.active { PINK } else { TEXT }),
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

        let icon_rect = if self.round {
            Rect::from_center_size(rect.center(), Vec2::splat(self.icon_size))
        } else {
            Rect::from_min_size(
                Pos2::new(rect.left() + padding.x, rect.center().y - self.icon_size / 2.0),
                Vec2::splat(self.icon_size),
            )
        };
        paint_icon(painter, icon_rect, self.icon, icon_color);
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

/// Bocchi's logo: her two cube hair clips.
pub fn logo(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let cube = size * 0.48;
    let draw = |min: Pos2, color: Color32| {
        let face = Rect::from_min_size(min, Vec2::splat(cube));
        painter.rect_filled(face, CornerRadius::same(3), color);
        // A darker edge on the right and bottom so it reads as a cube.
        let edge = Rect::from_min_max(Pos2::new(face.right() - cube * 0.22, face.top()), face.max);
        painter.rect_filled(edge, CornerRadius::same(3), color.gamma_multiply(0.7));
        let bottom = Rect::from_min_max(Pos2::new(face.left(), face.bottom() - cube * 0.22), face.max);
        painter.rect_filled(bottom, CornerRadius::same(3), color.gamma_multiply(0.7));
    };
    draw(rect.min + Vec2::new(0.0, size * 0.06), YELLOW);
    draw(rect.min + Vec2::new(size * 0.5, size * 0.46), BLUE);
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
