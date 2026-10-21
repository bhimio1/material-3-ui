//! The Appearance card every app's preferences page shows: color source,
//! light/dark, seed color, filled icons and interface scale.

use crate::common::{card, row};
use crate::theme::palette::{parse_hex, to_hex};
use crate::theme::{ColorScheme, ThemeManager, ThemeSettings, ThemeSource};
use crate::widgets::{self, segmented, switch};
use egui::{vec2, Color32, Ui};
use egui_material_icons::icons::*;

const SEED_PRESETS: &[&str] = &[
    "#4285F4", "#6750A4", "#B3261E", "#E8710A", "#1E8E3E", "#00897B", "#C2185B", "#795548",
    "#607D8B",
];

/// Draw the Appearance card. Returns true when `settings` changed (apply it
/// with [`ThemeManager::apply`] and save it).
pub fn appearance_card(ui: &mut Ui, settings: &mut ThemeSettings, theme: &ThemeManager) -> bool {
    let p = widgets::palette(ui);
    let mut changed = false;
    card(ui, "Appearance", |ui| {
        let desc = match theme.theme_file(settings) {
            Some(path) if Some(&path) == crate::theme::end4_colors_path().as_ref() => {
                "Following your end4-pC colors, live as the wallpaper changes.".to_string()
            }
            Some(path) => format!("Following {}.", crate::sys::paths::tilde(&path)),
            None if settings.source == ThemeSource::Auto => {
                "No desktop palette found, so the seed color below is used.".to_string()
            }
            None => "Using a palette generated from the seed color below.".to_string(),
        };
        row(ui, "Color source", &desc, |ui| {
            changed |= segmented(ui, &mut settings.source, &[
                (ThemeSource::Auto, ICON_AUTO_AWESOME, "Auto"),
                (ThemeSource::Seed, ICON_PALETTE, "Seed"),
            ]);
        });
        ui.separator();
        row(ui, "Color scheme", "Light, dark, or follow the system.", |ui| {
            changed |= segmented(ui, &mut settings.scheme, &[
                (ColorScheme::System, ICON_BRIGHTNESS_AUTO, "System"),
                (ColorScheme::Light, ICON_LIGHT_MODE, "Light"),
                (ColorScheme::Dark, ICON_DARK_MODE, "Dark"),
            ]);
        });
        ui.separator();
        row(ui, "Seed color", "Every color in the interface is derived from this one.", |ui| {
            let mut color = parse_hex(&settings.seed_color).unwrap_or(Color32::from_rgb(0x42, 0x85, 0xF4));
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.seed_color = to_hex(color);
                changed = true;
            }
            for hex in SEED_PRESETS.iter().rev() {
                let Some(c) = parse_hex(hex) else { continue };
                let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), egui::Sense::click());
                ui.painter().circle_filled(rect.center(), 11.0, c);
                if settings.seed_color.eq_ignore_ascii_case(hex) {
                    ui.painter().circle_stroke(rect.center(), 12.5, egui::Stroke::new(2.0, p.on_surface));
                }
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    settings.seed_color = hex.to_string();
                    changed = true;
                }
            }
        });
        ui.separator();
        row(ui, "Filled icons", "Use filled Material Symbols instead of outlined ones.", |ui| {
            changed |= switch(ui, &mut settings.filled_icons).changed();
        });
        ui.separator();
        row(ui, "Interface scale", "Size of everything in the window.", |ui| {
            let r = ui.add(
                egui::Slider::new(&mut settings.ui_scale, 0.75..=1.75)
                    .step_by(0.05)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            // Apply on release so the slider doesn't jump under the cursor.
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                changed = true;
            }
        });
    });
    changed
}
