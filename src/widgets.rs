//! Material 3 flavoured building blocks painted with egui primitives.

use crate::theme::palette::blend;
use crate::theme::M3Palette;
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};
use egui_material_icons::{icons::*, MaterialIcon};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static FILLED_ICONS: AtomicBool = AtomicBool::new(true);

pub fn set_filled_icons(filled: bool) {
    FILLED_ICONS.store(filled, Ordering::Relaxed);
}

fn styled(icon: MaterialIcon) -> MaterialIcon {
    if FILLED_ICONS.load(Ordering::Relaxed) {
        icon.filled()
    } else {
        icon.outlined()
    }
}

pub fn icon_font(icon: MaterialIcon, size: f32) -> FontId {
    FontId::new(size, styled(icon).font_family())
}

fn palette_id() -> Id {
    Id::unique("m3s-palette")
}

pub fn set_palette(ctx: &egui::Context, palette: M3Palette) {
    ctx.data_mut(|d| d.insert_temp(palette_id(), Arc::new(palette)));
}

pub fn palette(ui: &Ui) -> Arc<M3Palette> {
    ui.ctx()
        .data(|d| d.get_temp::<Arc<M3Palette>>(palette_id()))
        .unwrap_or_else(|| {
            Arc::new(M3Palette::from_seed(
                crate::theme::palette::DEFAULT_SEED,
                true,
            ))
        })
}

/// State layer color for hover/press per the M3 spec.
pub fn state_layer(base: Color32, on: Color32, response: &Response) -> Color32 {
    if response.is_pointer_button_down_on() {
        blend(base, on, 0.12)
    } else if response.hovered() || response.has_focus() {
        blend(base, on, 0.08)
    } else {
        base
    }
}

pub fn paint_icon(ui: &Ui, icon: MaterialIcon, center: egui::Pos2, size: f32, color: Color32) {
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        styled(icon).codepoint,
        icon_font(icon, size),
        color,
    );
}

/// A standalone icon, laid out like a label.
pub fn icon(ui: &mut Ui, icon: MaterialIcon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon(ui, icon, rect.center(), size, color);
    response
}

/// Rich text helper for putting an icon inline in widgets.
pub fn icon_text(icon: MaterialIcon, size: f32) -> egui::RichText {
    egui::RichText::new(styled(icon).codepoint)
        .family(styled(icon).font_family())
        .size(size)
}

/// Circular standard icon button (M3 "icon button").
pub fn icon_button(ui: &mut Ui, icon: MaterialIcon, tooltip: &str, enabled: bool) -> Response {
    icon_button_sized(ui, icon, tooltip, enabled, 36.0, false)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: MaterialIcon,
    tooltip: &str,
    enabled: bool,
    size: f32,
    selected: bool,
) -> Response {
    let p = palette(ui);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    if ui.is_rect_visible(rect) {
        let (bg, fg) = if selected {
            (p.secondary_container, p.on_secondary_container)
        } else {
            (Color32::TRANSPARENT, p.on_surface_variant)
        };
        let fill = if enabled {
            state_layer(bg, p.on_surface, &response)
        } else {
            bg
        };
        if fill != Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, CornerRadius::same((size / 2.0) as u8), fill);
        }
        let color = if enabled { fg } else { fg.gamma_multiply(0.38) };
        paint_icon(ui, icon, rect.center(), size * 0.58, color);
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Filled,
    Tonal,
    Outlined,
    Text,
    Danger,
}

/// Pill-shaped M3 button with optional leading icon.
pub fn button(ui: &mut Ui, kind: ButtonKind, icon: Option<MaterialIcon>, text: &str) -> Response {
    button_enabled(ui, kind, icon, text, true)
}

pub fn button_enabled(
    ui: &mut Ui,
    kind: ButtonKind,
    icon: Option<MaterialIcon>,
    text: &str,
    enabled: bool,
) -> Response {
    let p = palette(ui);
    let (bg, fg, stroke) = match kind {
        ButtonKind::Filled => (p.primary, p.on_primary, Stroke::NONE),
        ButtonKind::Tonal => (
            p.secondary_container,
            p.on_secondary_container,
            Stroke::NONE,
        ),
        ButtonKind::Outlined => (Color32::TRANSPARENT, p.primary, Stroke::new(1.0, p.outline)),
        ButtonKind::Text => (Color32::TRANSPARENT, p.primary, Stroke::NONE),
        ButtonKind::Danger => (p.error, p.on_error, Stroke::NONE),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), title_font(14.0), fg);
    let icon_w = if icon.is_some() { 18.0 + 8.0 } else { 0.0 };
    let pad_x = if kind == ButtonKind::Text { 12.0 } else { 20.0 };
    let size = vec2((galley.size().x + icon_w + pad_x * 2.0).max(64.0), 36.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(18);
        let (bg, fg) = if enabled {
            (bg, fg)
        } else {
            (
                if bg == Color32::TRANSPARENT {
                    bg
                } else {
                    p.on_surface.gamma_multiply(0.12)
                },
                p.on_surface.gamma_multiply(0.38),
            )
        };
        let fill = if !enabled {
            bg
        } else if bg == Color32::TRANSPARENT {
            if response.is_pointer_button_down_on() {
                fg.gamma_multiply(0.12)
            } else if response.hovered() || response.has_focus() {
                fg.gamma_multiply(0.08)
            } else {
                bg
            }
        } else {
            state_layer(bg, fg, &response)
        };
        ui.painter().rect_filled(rect, radius, fill);
        if stroke != Stroke::NONE {
            ui.painter()
                .rect_stroke(rect, radius, stroke, StrokeKind::Inside);
        }
        let content_w = galley.size().x + icon_w;
        let mut x = rect.center().x - content_w / 2.0;
        if let Some(icon) = icon {
            paint_icon(ui, icon, pos2(x + 9.0, rect.center().y), 18.0, fg);
            x += icon_w;
        }
        ui.painter()
            .galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// M3 filter chip: outlined when off, tonal with a check mark when on.
pub fn filter_chip(
    ui: &mut Ui,
    label: &str,
    selected: bool,
    leading: Option<MaterialIcon>,
) -> Response {
    let p = palette(ui);
    let fg = if selected {
        p.on_secondary_container
    } else {
        p.on_surface_variant
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), title_font(13.0), fg);
    let leading = if selected { Some(ICON_CHECK) } else { leading };
    let icon_w = if leading.is_some() { 18.0 + 6.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 24.0, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(8);
        let base = if selected {
            p.secondary_container
        } else {
            Color32::TRANSPARENT
        };
        let fill = if selected {
            state_layer(base, fg, &response)
        } else if response.hovered() {
            p.on_surface.gamma_multiply(0.08)
        } else {
            base
        };
        ui.painter().rect_filled(rect, radius, fill);
        if !selected {
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, p.outline_variant),
                StrokeKind::Inside,
            );
        }
        let mut x = rect.left() + 12.0;
        if let Some(icon) = leading {
            paint_icon(ui, icon, pos2(x + 9.0, rect.center().y), 18.0, fg);
            x += icon_w;
        }
        ui.painter()
            .galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Navigation drawer destination: full-width pill.
pub fn nav_item(
    ui: &mut Ui,
    icon: MaterialIcon,
    label: &str,
    selected: bool,
    trailing: Option<&str>,
) -> Response {
    let p = palette(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 40.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let (base, fg, icon_color) = if selected {
            (
                p.secondary_container,
                p.on_secondary_container,
                p.on_secondary_container,
            )
        } else {
            (
                Color32::TRANSPARENT,
                p.on_surface_variant,
                p.on_surface_variant,
            )
        };
        let fill = if selected {
            state_layer(base, fg, &response)
        } else if response.hovered() {
            p.on_surface
                .gamma_multiply(if response.is_pointer_button_down_on() {
                    0.12
                } else {
                    0.08
                })
        } else {
            base
        };
        ui.painter().rect_filled(rect, CornerRadius::same(20), fill);
        paint_icon(
            ui,
            icon,
            pos2(rect.left() + 26.0, rect.center().y),
            22.0,
            icon_color,
        );
        let max_text = rect.width() - 56.0 - if trailing.is_some() { 40.0 } else { 12.0 };
        let galley = truncated_galley(ui, label, title_font(14.0), fg, max_text);
        ui.painter().galley(
            pos2(rect.left() + 50.0, rect.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
        if let Some(t) = trailing {
            ui.painter().text(
                pos2(rect.right() - 16.0, rect.center().y),
                Align2::RIGHT_CENTER,
                t,
                FontId::proportional(12.0),
                p.on_surface_variant,
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Single-line galley cut with an ellipsis to fit `max_width`.
pub fn truncated_galley(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat::simple(font, color),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(10.0));
    ui.painter().layout_job(job)
}

pub fn section_header(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(title_text(text, 13.0).color(p.on_surface_variant));
    });
    ui.add_space(2.0);
}

/// M3 switch (52×32 track).
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let p = palette(ui);
    let (rect, mut response) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(response.id, *on, 0.15);
    if ui.is_rect_visible(rect) {
        let track = blend(p.surface_container_highest, p.primary, t);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(16), track);
        if t < 1.0 {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(16),
                Stroke::new(2.0, p.outline.gamma_multiply(1.0 - t)),
                StrokeKind::Inside,
            );
        }
        let radius = egui::lerp(8.0..=12.0, t)
            + if response.is_pointer_button_down_on() {
                2.0
            } else {
                0.0
            };
        let cx = egui::lerp((rect.left() + 16.0)..=(rect.right() - 16.0), t);
        let center = pos2(cx, rect.center().y);
        if response.hovered() {
            ui.painter()
                .circle_filled(center, 20.0, p.on_surface.gamma_multiply(0.08));
        }
        let knob = blend(p.outline, p.on_primary, t);
        ui.painter().circle_filled(center, radius, knob);
        if t > 0.5 {
            paint_icon(
                ui,
                ICON_CHECK,
                center,
                16.0,
                p.on_primary_container.gamma_multiply((t - 0.5) * 2.0),
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Segmented button group. Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, MaterialIcon, &str)],
) -> bool {
    let p = palette(ui);
    let mut changed = false;
    let seg_w = |label: &str, ui: &Ui| {
        if label.is_empty() {
            44.0
        } else {
            ui.painter()
                .layout_no_wrap(label.to_string(), title_font(13.0), Color32::WHITE)
                .size()
                .x
                + 54.0
        }
    };
    let widths: Vec<f32> = options.iter().map(|(_, _, l)| seg_w(l, ui)).collect();
    let total: f32 = widths.iter().sum();
    let (rect, group) = ui.allocate_exact_size(vec2(total, 34.0), Sense::hover());
    let mut x = rect.left();
    for (i, (opt, icon, label)) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top()), vec2(widths[i], rect.height()));
        x += widths[i];
        let id = group.id.with(("seg", i));
        let response = ui
            .interact(r, id, Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = *value == *opt;
        let radius = CornerRadius {
            nw: if i == 0 { 17 } else { 0 },
            sw: if i == 0 { 17 } else { 0 },
            ne: if i + 1 == options.len() { 17 } else { 0 },
            se: if i + 1 == options.len() { 17 } else { 0 },
        };
        let base = if selected {
            p.secondary_container
        } else {
            Color32::TRANSPARENT
        };
        let fg = if selected {
            p.on_secondary_container
        } else {
            p.on_surface
        };
        let fill = if selected {
            state_layer(base, fg, &response)
        } else if response.hovered() {
            p.on_surface.gamma_multiply(0.08)
        } else {
            base
        };
        ui.painter().rect_filled(r, radius, fill);
        ui.painter()
            .rect_stroke(r, radius, Stroke::new(1.0, p.outline), StrokeKind::Inside);
        if label.is_empty() {
            paint_icon(ui, *icon, r.center(), 20.0, fg);
        } else {
            let icon = if selected { ICON_CHECK } else { *icon };
            paint_icon(ui, icon, pos2(r.left() + 22.0, r.center().y), 18.0, fg);
            ui.painter().text(
                pos2(r.left() + 36.0, r.center().y),
                Align2::LEFT_CENTER,
                *label,
                title_font(13.0),
                fg,
            );
        }
        if response.clicked() && !selected {
            *value = *opt;
            changed = true;
        }
    }
    changed
}

pub fn linear_progress(ui: &mut Ui, fraction: Option<f32>, width: f32) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 4.0), Sense::hover());
    let radius = CornerRadius::same(2);
    match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            let split = rect.left() + rect.width() * f;
            let filled = Rect::from_min_max(rect.min, pos2(split, rect.max.y));
            let track =
                Rect::from_min_max(pos2((split + 4.0).min(rect.right()), rect.min.y), rect.max);
            ui.painter()
                .rect_filled(track, radius, p.secondary_container);
            ui.painter().rect_filled(filled, radius, p.primary);
        }
        None => {
            ui.painter()
                .rect_filled(rect, radius, p.secondary_container);
            let t = (ui.input(|i| i.time) * 0.8).fract() as f32;
            let start = rect.left() + rect.width() * (t * 1.4 - 0.4);
            let bar = Rect::from_min_max(
                pos2(start.max(rect.left()), rect.top()),
                pos2(
                    (start + rect.width() * 0.4).min(rect.right()),
                    rect.bottom(),
                ),
            );
            if bar.width() > 0.0 {
                ui.painter().rect_filled(bar, radius, p.primary);
            }
            ui.ctx().request_repaint();
        }
    }
    response
}

/// A clickable card with a hover state layer.
pub fn clickable_card(ui: &mut Ui, size: Vec2, fill: Color32, id: Id) -> (Rect, Response) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = state_layer(fill, p.on_surface, &response);
    ui.painter().rect_filled(rect, CornerRadius::same(16), fill);
    (rect, response)
}

/// Text in the medium-weight title font.
pub fn title_text(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text)
        .size(size)
        .family(crate::theme::style::medium_family())
}

pub fn title_font(size: f32) -> FontId {
    FontId::new(size, crate::theme::style::medium_family())
}

/// A round color swatch with a ring when selected.
pub fn color_swatch(ui: &mut Ui, color: Color32, selected: bool, tooltip: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        if selected {
            ui.painter().circle_stroke(c, 13.0, Stroke::new(2.0, p.primary));
        } else if response.hovered() {
            ui.painter().circle_stroke(c, 13.0, Stroke::new(1.5, p.outline));
        }
        ui.painter().circle_filled(c, 9.5, color);
        ui.painter().circle_stroke(c, 9.5, Stroke::new(1.0, p.outline_variant));
    }
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// A big tile with an icon and two lines of text, for a page's main
/// actions. `accent` makes it primary-tinted.
pub fn action_tile(ui: &mut Ui, icon: MaterialIcon, title: &str, subtitle: &str, width: f32, accent: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 112.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let (base, fg, icon_fg, sub_fg) = if accent {
            (p.primary_container, p.on_primary_container, p.on_primary_container, p.on_primary_container.gamma_multiply(0.8))
        } else {
            (p.surface_container_high, p.on_surface, p.primary, p.on_surface_variant)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(20), state_layer(base, fg, &response));
        paint_icon(ui, icon, rect.left_top() + vec2(30.0, 34.0), 28.0, icon_fg);
        let text_w = width - 32.0;
        let title_g = truncated_galley(ui, title, title_font(16.0), fg, text_w);
        ui.painter().galley(rect.left_bottom() + vec2(16.0, -38.0 - title_g.size().y / 2.0), title_g, fg);
        let sub_g = truncated_galley(ui, subtitle, FontId::proportional(12.5), sub_fg, text_w);
        ui.painter().galley(rect.left_bottom() + vec2(16.0, -18.0 - sub_g.size().y / 2.0), sub_g, sub_fg);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A thumbnail card: the picture (or `fallback` while it loads), a title
/// and a subtitle, and an optional badge in the picture's corner.
pub fn thumb_tile(
    ui: &mut Ui,
    texture: Option<&egui::TextureHandle>,
    fallback: MaterialIcon,
    title: &str,
    subtitle: &str,
    width: f32,
    badge: Option<MaterialIcon>,
) -> Response {
    let p = palette(ui);
    let pic_h = (width * 0.62).round();
    let (rect, response) = ui.allocate_exact_size(vec2(width, pic_h + 52.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    ui.painter().rect_filled(rect, CornerRadius::same(16), state_layer(p.surface_container, p.on_surface, &response));
    let pic = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(width - 12.0, pic_h - 6.0));
    ui.painter().rect_filled(pic, CornerRadius::same(12), p.surface_container_highest);
    match texture {
        Some(tex) => {
            // Fit inside, keeping the aspect ratio.
            let ts = tex.size_vec2();
            let s = (pic.width() / ts.x).min(pic.height() / ts.y);
            let r = Rect::from_center_size(pic.center(), ts * s);
            egui::Image::new(tex).corner_radius(CornerRadius::same(8)).paint_at(ui, r);
        }
        None => paint_icon(ui, fallback, pic.center(), 32.0, p.on_surface_variant),
    }
    if let Some(b) = badge {
        let c = pic.right_top() + vec2(-18.0, 18.0);
        ui.painter().circle_filled(c, 13.0, p.secondary_container);
        paint_icon(ui, b, c, 16.0, p.on_secondary_container);
    }
    let text_w = width - 24.0;
    let title_g = truncated_galley(ui, title, FontId::proportional(13.5), p.on_surface, text_w);
    ui.painter().galley(rect.left_bottom() + vec2(12.0, -42.0), title_g, p.on_surface);
    let sub_g = truncated_galley(ui, subtitle, FontId::proportional(12.0), p.on_surface_variant, text_w);
    ui.painter().galley(rect.left_bottom() + vec2(12.0, -22.0), sub_g, p.on_surface_variant);
    if response.hovered() {
        ui.painter().rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0, p.outline_variant), StrokeKind::Inside);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A rounded single-line text field. Returns the TextEdit's response.
pub fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32, id: Id) -> Response {
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.surface_container_highest)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .hint_text(hint)
                    .font(FontId::proportional(14.0))
                    .desired_width(width - 20.0),
            )
        })
        .inner
}

/// A compact text button for status bars: label, hover layer, tooltip.
pub fn chip(ui: &mut Ui, text: &str, tooltip: &str) -> Response {
    let p = palette(ui);
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), p.on_surface_variant);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + 16.0, 24.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(12), p.on_surface.gamma_multiply(0.08));
    }
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.on_surface_variant);
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// A menu entry with an optional leading icon, a check mark when
/// `checked`, and a shortcut hint on the right. Closes the menu when
/// clicked; returns whether it was.
pub fn menu_item(ui: &mut Ui, icon: Option<MaterialIcon>, text: &str, shortcut: &str, checked: bool) -> bool {
    let p = palette(ui);
    // Context menus offer the whole window's width; keep them compact.
    let width = ui.available_width().clamp(220.0, 300.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), p.on_surface.gamma_multiply(0.08));
    }
    let lead = if checked { Some(ICON_CHECK) } else { icon };
    if let Some(i) = lead {
        let color = if checked { p.primary } else { p.on_surface_variant };
        paint_icon(ui, i, pos2(rect.left() + 19.0, rect.center().y), 18.0, color);
    }
    let hint_w = if shortcut.is_empty() {
        0.0
    } else {
        let g = ui.painter().layout_no_wrap(shortcut.to_string(), FontId::proportional(12.0), p.on_surface_variant);
        let w = g.size().x;
        ui.painter().galley(pos2(rect.right() - 10.0 - w, rect.center().y - g.size().y / 2.0), g, p.on_surface_variant);
        w + 16.0
    };
    let label = truncated_galley(ui, text, FontId::proportional(14.0), p.on_surface, rect.width() - 48.0 - hint_w);
    ui.painter().galley(pos2(rect.left() + 38.0, rect.center().y - label.size().y / 2.0), label, p.on_surface);
    let clicked = response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// An icon for a file, by name (or a folder, open when `expanded`).
pub fn file_icon(name: &str, is_dir: bool, expanded: bool) -> MaterialIcon {
    if is_dir {
        return if expanded { ICON_FOLDER_OPEN } else { ICON_FOLDER };
    }
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" | "c" | "h" | "cc" | "cpp" | "hpp" | "go" | "py" | "java" | "kt" | "rb" | "zig" | "lua" | "qml" | "nim"
        | "swift" => ICON_CODE,
        "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" => ICON_JAVASCRIPT,
        "json" | "jsonc" | "json5" => ICON_DATA_OBJECT,
        "html" | "htm" | "xml" | "svg" => ICON_HTML,
        "css" | "scss" | "sass" | "less" => ICON_CSS,
        "md" | "markdown" | "mdown" => ICON_MARKDOWN,
        "sh" | "bash" | "zsh" | "fish" => ICON_TERMINAL,
        "toml" | "yaml" | "yml" | "ini" | "conf" | "cfg" | "desktop" | "service" | "env" => ICON_SETTINGS,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" => ICON_IMAGE,
        "pdf" => ICON_PICTURE_AS_PDF,
        "mp3" | "flac" | "ogg" | "oga" | "wav" | "opus" | "m4a" => ICON_AUDIO_FILE,
        "mp4" | "mkv" | "webm" | "mov" | "avi" => ICON_VIDEO_FILE,
        "zip" | "tar" | "gz" | "xz" | "zst" | "7z" | "rar" | "bz2" => ICON_FOLDER_ZIP,
        "lock" => ICON_LOCK,
        "sql" | "db" | "sqlite" => ICON_DATABASE,
        "txt" | "log" => ICON_ARTICLE,
        _ if matches!(lower.as_str(), "pkgbuild" | "makefile" | "dockerfile" | "justfile") => ICON_DEPLOYED_CODE,
        _ => ICON_DESCRIPTION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_by_extension() {
        let same = |a: MaterialIcon, b: MaterialIcon| a.codepoint == b.codepoint;
        assert!(same(file_icon("main.rs", false, false), ICON_CODE));
        assert!(same(file_icon("README.MD", false, false), ICON_MARKDOWN));
        assert!(same(file_icon("PKGBUILD", false, false), ICON_DEPLOYED_CODE));
        assert!(same(file_icon("src", true, true), ICON_FOLDER_OPEN));
        assert!(same(file_icon("src", true, false), ICON_FOLDER));
        assert!(same(file_icon("noext", false, false), ICON_DESCRIPTION));
        assert!(same(file_icon("archive.tar.gz", false, false), ICON_FOLDER_ZIP));
    }
}
