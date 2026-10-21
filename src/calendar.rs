//! The mini-month calendar widget (a sidebar date picker) and the date
//! math behind it.

use crate::widgets;
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use egui::{pos2, vec2, Align2, FontId, Rect, Sense, Stroke, Ui};
use egui_material_icons::icons::*;

/// `d` moved by `n` months, the day clamped to the month's length
/// (Jan 31 + 1 month = Feb 28).
pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + n;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let mut day = d.day();
    loop {
        if let Some(x) = NaiveDate::from_ymd_opt(y, m, day) {
            return x;
        }
        day -= 1;
    }
}

/// The first day of the week containing `d`.
pub fn week_start(d: NaiveDate, first: Weekday) -> NaiveDate {
    let back = (d.weekday().num_days_from_monday() as i64 - first.num_days_from_monday() as i64).rem_euclid(7);
    d - Duration::days(back)
}

/// The first of the six weeks a month grid shows.
pub fn month_grid_start(d: NaiveDate, first: Weekday) -> NaiveDate {
    week_start(d.with_day(1).unwrap_or(d), first)
}

/// Short weekday names ("Mon", …) starting at `first`.
pub fn weekday_names(first: Weekday) -> [String; 7] {
    let start = week_start(NaiveDate::from_ymd_opt(2024, 1, 3).unwrap_or_default(), first);
    std::array::from_fn(|i| (start + Duration::days(i as i64)).format("%a").to_string())
}

/// A mini month: the month's name with previous/next buttons, then six
/// weeks of days. Today is filled with the primary color, `selected` gets a
/// tonal circle, and days where `marked` is true get a dot (days with
/// events). `month` is any day of the month shown; the arrows change it.
/// Returns the day clicked; clicking a day of the next or previous month
/// also moves `month` there.
pub fn mini_month(
    ui: &mut Ui,
    month: &mut NaiveDate,
    selected: NaiveDate,
    first: Weekday,
    marked: impl Fn(NaiveDate) -> bool,
) -> Option<NaiveDate> {
    let p = widgets::palette(ui);
    let shown = month.with_day(1).unwrap_or(*month);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(widgets::title_text(shown.format("%B %Y").to_string(), 14.0).color(p.on_surface));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button_sized(ui, ICON_CHEVRON_RIGHT, "Next month", true, 30.0, false).clicked() {
                *month = add_months(shown, 1);
            }
            if widgets::icon_button_sized(ui, ICON_CHEVRON_LEFT, "Previous month", true, 30.0, false).clicked() {
                *month = add_months(shown, -1);
            }
        });
    });
    let w = ui.available_width();
    let cell = (w / 7.0).floor();
    let row_h = cell * 0.86;
    let (rect, grid) = ui.allocate_exact_size(vec2(w, cell * 0.8 + row_h * 6.0), Sense::hover());
    for (i, n) in weekday_names(first).iter().enumerate() {
        ui.painter().text(
            pos2(rect.left() + cell * (i as f32 + 0.5), rect.top() + cell * 0.4),
            Align2::CENTER_CENTER,
            n.chars().next().unwrap_or(' ').to_string(),
            widgets::title_font(11.0),
            p.on_surface_variant,
        );
    }
    let start = month_grid_start(shown, first);
    let today = Local::now().date_naive();
    let mut clicked = None;
    for i in 0..42 {
        let d = start + Duration::days(i);
        let c = pos2(
            rect.left() + cell * ((i % 7) as f32 + 0.5),
            rect.top() + cell * 0.8 + row_h * ((i / 7) as f32 + 0.5),
        );
        let r = Rect::from_center_size(c, vec2(cell, row_h));
        let resp = ui.interact(r, grid.id.with(("day", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let radius = (row_h / 2.0 - 1.0).min(15.0);
        let is_selected = d == selected;
        let fg = if d == today {
            ui.painter().circle_filled(c, radius, p.primary);
            p.on_primary
        } else if is_selected {
            ui.painter().circle_filled(c, radius, p.secondary_container);
            p.on_secondary_container
        } else {
            if resp.hovered() {
                ui.painter().circle_filled(c, radius, p.on_surface.gamma_multiply(0.08));
            }
            if d.month() == shown.month() {
                p.on_surface
            } else {
                p.on_surface_variant.gamma_multiply(0.45)
            }
        };
        if is_selected && d == today {
            ui.painter().circle_stroke(c, radius + 2.0, Stroke::new(1.5, p.primary));
        }
        ui.painter().text(c, Align2::CENTER_CENTER, d.day().to_string(), FontId::proportional(12.0), fg);
        if d != today && marked(d) {
            let dot = if is_selected { p.on_secondary_container } else { p.primary };
            ui.painter().circle_filled(pos2(c.x, c.y + radius - 3.0), 1.8, dot);
        }
        if resp.clicked() {
            clicked = Some(d);
            if d.month() != shown.month() {
                *month = d.with_day(1).unwrap_or(d);
            }
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn date_math() {
        assert_eq!(add_months(day(2026, 1, 31), 1), day(2026, 2, 28));
        assert_eq!(add_months(day(2026, 1, 31), -2), day(2025, 11, 30));
        assert_eq!(add_months(day(2024, 3, 31), -1), day(2024, 2, 29));
        assert_eq!(add_months(day(2026, 12, 5), 1), day(2027, 1, 5));
        let wed = day(2026, 10, 21);
        assert_eq!(week_start(wed, Weekday::Mon), day(2026, 10, 19));
        assert_eq!(week_start(wed, Weekday::Sun), day(2026, 10, 18));
        assert_eq!(week_start(wed, Weekday::Sat), day(2026, 10, 17));
        assert_eq!(week_start(day(2026, 10, 19), Weekday::Mon), day(2026, 10, 19));
        assert_eq!(month_grid_start(wed, Weekday::Mon), day(2026, 9, 28));
        assert_eq!(weekday_names(Weekday::Sun)[0], "Sun");
        assert_eq!(weekday_names(Weekday::Mon)[6], "Sun");
    }

    #[test]
    fn renders_and_keeps_the_month() {
        let ctx = egui::Context::default();
        let mut month = day(2026, 10, 21);
        for _ in 0..2 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                assert_eq!(mini_month(ui, &mut month, day(2026, 10, 21), Weekday::Mon, |d| d.day() == 3), None);
            })
            .drop_without_applying_deltas();
        }
        assert_eq!(month, day(2026, 10, 21));
    }
}
