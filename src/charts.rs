//! Small live charts for dashboards: an area chart of recent history,
//! usage meters, per-core bars and legends.

use crate::widgets;
use egui::{pos2, vec2, Color32, CornerRadius, Mesh, Rect, Response, RichText, Sense, Shape, Stroke, Ui, Vec2};

/// One line of an [`area_chart`].
pub struct Series<'a> {
    pub values: &'a [f32],
    pub color: Color32,
}

/// A rounded "nice" maximum for an auto-scaled chart (1, 2, 5 × 10ⁿ).
pub fn nice_max(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return 1.0;
    }
    let exp = v.log10().floor();
    let base = 10f32.powf(exp);
    let m = v / base;
    let step = if m <= 1.0 {
        1.0
    } else if m <= 2.0 {
        2.0
    } else if m <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * base
}

/// Where a chart's samples go: the newest at the right edge, `capacity`
/// samples across `plot`, values scaled to `max`.
pub fn chart_points(values: &[f32], plot: Rect, max: f32, capacity: usize) -> Vec<egui::Pos2> {
    let max = if max > 0.0 && max.is_finite() { max } else { 1.0 };
    let capacity = capacity.max(2);
    let n = values.len().min(capacity);
    let step = plot.width() / (capacity - 1) as f32;
    values[values.len() - n..]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let v = if v.is_finite() { *v } else { 0.0 };
            let x = plot.right() - (n - 1 - i) as f32 * step;
            let y = plot.bottom() - (v / max).clamp(0.0, 1.0) * plot.height();
            pos2(x, y)
        })
        .collect()
}

/// A live area chart: newest value at the right edge, `capacity` samples
/// across, a gradient fill under each line and faint grid lines.
pub fn area_chart(ui: &mut Ui, size: Vec2, series: &[Series], max: f32, capacity: usize) -> Response {
    let p = widgets::palette(ui);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(12), p.surface_container_high);
    let plot = rect.shrink2(vec2(2.0, 6.0));
    for i in 1..4 {
        let y = plot.bottom() - plot.height() * i as f32 / 4.0;
        painter.line_segment(
            [pos2(plot.left() + 6.0, y), pos2(plot.right() - 6.0, y)],
            Stroke::new(1.0, p.outline_variant.gamma_multiply(0.35)),
        );
    }
    for s in series {
        let points = chart_points(s.values, plot, max, capacity);
        if points.len() < 2 {
            continue;
        }
        let mut mesh = Mesh::default();
        let top = s.color.gamma_multiply(0.30);
        let bottom = s.color.gamma_multiply(0.04);
        for w in points.windows(2) {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(w[0], top);
            mesh.colored_vertex(w[1], top);
            mesh.colored_vertex(pos2(w[1].x, plot.bottom()), bottom);
            mesh.colored_vertex(pos2(w[0].x, plot.bottom()), bottom);
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::line(points, Stroke::new(1.8, s.color)));
    }
    response
}

/// A horizontal usage meter; turns to the error color above 90%.
pub fn meter(ui: &mut Ui, fraction: f32, width: f32) -> Response {
    let p = widgets::palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 8.0), Sense::hover());
    let f = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 };
    let r = CornerRadius::same(4);
    ui.painter().rect_filled(rect, r, p.secondary_container);
    if f > 0.0 {
        let fill = Rect::from_min_size(rect.min, vec2((rect.width() * f).max(8.0), rect.height()));
        ui.painter().rect_filled(fill, r, if f > 0.9 { p.error } else { p.primary });
    }
    response
}

/// Vertical bars, one per value (0–1), filling the width: per-core load.
/// Bars above 90% use the tertiary color.
pub fn bars(ui: &mut Ui, values: &[f32], height: f32) -> Response {
    let p = widgets::palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if values.is_empty() || !ui.is_rect_visible(rect) {
        return response;
    }
    let gap = if values.len() > 32 { 2.0 } else { 4.0 };
    let w = ((rect.width() + gap) / values.len() as f32 - gap).max(1.0);
    for (i, v) in values.iter().enumerate() {
        let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
        let x = rect.left() + i as f32 * (w + gap);
        let col = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
        let radius = CornerRadius::same((w / 2.0).min(4.0) as u8);
        ui.painter().rect_filled(col, radius, p.secondary_container);
        let h = (v * rect.height()).max(2.0);
        let fill = Rect::from_min_max(pos2(col.left(), col.bottom() - h), col.max);
        ui.painter().rect_filled(fill, radius, if v > 0.9 { p.tertiary } else { p.primary });
    }
    response
}

/// A colored dot and label, for chart legends. Lay several out in a
/// `ui.horizontal`.
pub fn legend(ui: &mut Ui, color: Color32, text: &str) {
    let p = widgets::palette(ui);
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.0, color);
    ui.label(RichText::new(text).size(12.5).color(p.on_surface_variant));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_maxima() {
        assert_eq!(nice_max(0.0), 1.0);
        assert_eq!(nice_max(f32::NAN), 1.0);
        assert_eq!(nice_max(130.0), 200.0);
        assert_eq!(nice_max(3.2e6), 5e6);
        assert_eq!(nice_max(1000.0), 1000.0);
        assert_eq!(nice_max(7.0), 10.0);
    }

    #[test]
    fn points_hug_the_right_edge() {
        let plot = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let pts = chart_points(&[0.0, 5.0, 10.0, 20.0], plot, 10.0, 11);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[3].x, 100.0, "newest at the right");
        assert_eq!(pts[2].x, 90.0);
        assert_eq!(pts[0].y, 50.0, "zero at the bottom");
        assert_eq!(pts[1].y, 25.0);
        assert_eq!(pts[3].y, 0.0, "clamped to the top");
        // Only the newest `capacity` samples are kept.
        let pts = chart_points(&[1.0; 30], plot, 0.0, 5);
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0].x, 0.0);
    }
}
