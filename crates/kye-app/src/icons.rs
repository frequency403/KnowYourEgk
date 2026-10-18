//! Line icons and small illustrations, painted with egui shapes so they follow
//! the palette in light and dark mode. Coordinates are given in a design grid
//! (e.g. 22×22) and scaled into the allocated rect.

use std::f32::consts::PI;

use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Sense, Shape, Stroke, Vec2};

/// Maps design-grid coordinates into a screen rect.
struct Grid {
    rect: Rect,
    scale: f32,
}

impl Grid {
    fn new(rect: Rect, design: Vec2) -> Self {
        let scale = (rect.width() / design.x).min(rect.height() / design.y);
        let size = design * scale;
        let rect = Rect::from_center_size(rect.center(), size);
        Self { rect, scale }
    }

    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + Vec2::new(x, y) * self.scale
    }

    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), Vec2::new(w, h) * self.scale)
    }

    fn s(&self, width: f32, color: Color32) -> Stroke {
        Stroke::new(width * self.scale, color)
    }

    fn radius(&self, r: f32) -> CornerRadius {
        CornerRadius::same((r * self.scale).round().clamp(0.0, 255.0) as u8)
    }
}

fn arc(center: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    let steps = 24;
    (0..=steps)
        .map(|i| {
            let t = from + (to - from) * i as f32 / steps as f32;
            center + Vec2::new(t.cos(), t.sin()) * radius
        })
        .collect()
}

/// Allocates `size` and paints with `paint`; returns the response for tooltips.
pub fn paint(ui: &mut egui::Ui, size: Vec2, paint: impl FnOnce(&Painter, Rect)) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    paint(ui.painter(), rect);
    response
}

/// Smart card in a contact reader.
pub fn contact(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(22.0));
    p.rect_stroke(
        g.r(3.0, 4.0, 16.0, 14.0),
        g.radius(2.5),
        g.s(1.6, c),
        egui::StrokeKind::Middle,
    );
    let s = g.s(1.3, c);
    for (a, b) in [
        ((3.0, 9.0), (8.0, 9.0)),
        ((3.0, 13.0), (8.0, 13.0)),
        ((14.0, 9.0), (19.0, 9.0)),
        ((14.0, 13.0), (19.0, 13.0)),
        ((8.0, 4.0), (8.0, 18.0)),
        ((14.0, 4.0), (14.0, 18.0)),
    ] {
        p.line_segment([g.p(a.0, a.1), g.p(b.0, b.1)], s);
    }
}

/// Contactless (NFC) waves.
pub fn nfc(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(22.0));
    let center = g.p(4.0, 11.0);
    p.circle_filled(center, 1.6 * g.scale, c);
    for r in [4.5, 8.0, 11.5] {
        p.add(Shape::line(
            arc(center, r * g.scale, -0.28 * PI, 0.28 * PI),
            g.s(1.6, c),
        ));
    }
}

/// Padlock.
pub fn lock(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(16.0));
    p.rect_stroke(
        g.r(3.0, 7.0, 10.0, 7.0),
        g.radius(1.5),
        g.s(1.4, c),
        egui::StrokeKind::Middle,
    );
    let mut pts = vec![g.p(5.5, 7.0)];
    pts.extend(arc(g.p(8.0, 5.0), 2.5 * g.scale, PI, 2.0 * PI));
    pts.push(g.p(10.5, 7.0));
    p.add(Shape::line(pts, g.s(1.4, c)));
}

/// Check mark.
pub fn check(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(14.0));
    p.add(Shape::line(
        vec![g.p(3.0, 7.5), g.p(5.5, 10.0), g.p(11.0, 4.5)],
        g.s(2.0, c),
    ));
}

/// Warning triangle.
pub fn alert(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(22.0));
    p.add(Shape::closed_line(
        vec![g.p(11.0, 3.0), g.p(20.0, 19.0), g.p(2.0, 19.0)],
        g.s(1.8, c),
    ));
    p.line_segment([g.p(11.0, 9.0), g.p(11.0, 13.5)], g.s(1.8, c));
    p.circle_filled(g.p(11.0, 16.3), 1.0 * g.scale, c);
}

/// Exclamation mark in a circle.
pub fn info(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(20.0));
    p.circle_stroke(g.p(10.0, 10.0), 8.0 * g.scale, g.s(1.6, c));
    p.line_segment([g.p(10.0, 5.5), g.p(10.0, 11.0)], g.s(1.8, c));
    p.circle_filled(g.p(10.0, 14.0), 1.0 * g.scale, c);
}

/// App logo: a card with chip and text lines.
pub fn logo(p: &Painter, rect: Rect, c: Color32) {
    let g = Grid::new(rect, Vec2::splat(32.0));
    p.rect_stroke(
        g.r(2.0, 6.0, 28.0, 20.0),
        g.radius(3.0),
        g.s(2.0, c),
        egui::StrokeKind::Middle,
    );
    p.rect_stroke(
        g.r(7.0, 11.0, 8.0, 7.0),
        g.radius(1.5),
        g.s(1.6, c),
        egui::StrokeKind::Middle,
    );
    p.line_segment([g.p(19.0, 13.0), g.p(26.0, 13.0)], g.s(1.6, c));
    p.line_segment([g.p(19.0, 17.0), g.p(24.0, 17.0)], g.s(1.6, c));
}

/// Card front with the CAN field highlighted (top right).
pub fn can_card(p: &Painter, rect: Rect, pal: &crate::theme::Palette) {
    let g = Grid::new(rect, Vec2::new(64.0, 42.0));
    p.rect(
        g.r(1.0, 1.0, 62.0, 40.0),
        g.radius(5.0),
        pal.surface,
        g.s(1.0, pal.faint),
        egui::StrokeKind::Middle,
    );
    p.rect_filled(g.r(8.0, 14.0, 11.0, 9.0), g.radius(2.0), pal.chip);
    p.rect(
        g.r(40.0, 5.0, 18.0, 7.0),
        g.radius(2.0),
        pal.accent_soft,
        g.s(1.4, pal.accent),
        egui::StrokeKind::Middle,
    );
    p.line_segment(
        [g.p(8.0, 30.0), g.p(30.0, 30.0)],
        g.s(2.0, pal.control_border),
    );
    p.line_segment(
        [g.p(8.0, 34.0), g.p(22.0, 34.0)],
        g.s(2.0, pal.control_border),
    );
}

/// Reader with a card being inserted (empty state).
pub fn reader_with_card(p: &Painter, rect: Rect, pal: &crate::theme::Palette) {
    let g = Grid::new(rect, Vec2::new(180.0, 120.0));
    p.rect(
        g.r(10.0, 62.0, 160.0, 44.0),
        g.radius(10.0),
        pal.surface,
        g.s(2.0, pal.faint),
        egui::StrokeKind::Middle,
    );
    p.rect_filled(g.r(40.0, 74.0, 100.0, 8.0), g.radius(4.0), pal.line);
    p.rect(
        g.r(52.0, 12.0, 76.0, 50.0),
        g.radius(6.0),
        pal.surface,
        g.s(2.0, pal.accent),
        egui::StrokeKind::Middle,
    );
    p.rect_filled(g.r(62.0, 28.0, 14.0, 11.0), g.radius(2.0), pal.chip);
    p.line_segment(
        [g.p(84.0, 30.0), g.p(118.0, 30.0)],
        g.s(3.0, pal.control_border),
    );
    p.line_segment(
        [g.p(84.0, 37.0), g.p(108.0, 37.0)],
        g.s(3.0, pal.control_border),
    );
    let s = g.s(2.0, pal.accent);
    p.line_segment([g.p(90.0, 4.0), g.p(90.0, 8.0)], s);
    p.add(Shape::line(
        vec![g.p(86.0, 8.0), g.p(90.0, 4.0), g.p(94.0, 8.0)],
        s,
    ));
}
