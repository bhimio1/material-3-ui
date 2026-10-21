//! Every widget in one window: `cargo run --example gallery`.

use material_3_ui::common::{self, card, row};
use material_3_ui::icons::icons::*;
use material_3_ui::prefs;
use material_3_ui::theme::{ThemeManager, ThemeSettings};
use material_3_ui::toasts::Toasts;
use material_3_ui::widgets::{self, ButtonKind};

#[derive(Clone, Copy, PartialEq)]
enum View {
    Grid,
    List,
}

struct Gallery {
    settings: ThemeSettings,
    theme: ThemeManager,
    toasts: Toasts<()>,
    page: usize,
    on: bool,
    view: View,
    query: String,
    chip: bool,
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.theme.poll(&self.settings);
        let p = self.theme.palette().clone();
        let surface = egui::Frame::new().fill(p.surface);
        egui::Panel::left("nav")
            .default_size(240.0)
            .show_separator_line(false)
            .frame(surface.inner_margin(egui::Margin::same(12)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::icon(ui, ICON_PALETTE, 26.0, p.primary);
                    ui.label(widgets::title_text("Gallery", 20.0));
                });
                ui.add_space(12.0);
                common::search_field(ui, &mut self.query, "Search", ui.available_width());
                widgets::section_header(ui, "Pages");
                for (i, (icon, name)) in [(ICON_WIDGETS, "Widgets"), (ICON_SETTINGS, "Appearance")].into_iter().enumerate() {
                    if widgets::nav_item(ui, icon, name, self.page == i, None).clicked() {
                        self.page = i;
                    }
                }
            });
        egui::CentralPanel::default()
            .frame(surface.inner_margin(egui::Margin::same(12)))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(p.surface_container_low)
                    .corner_radius(egui::CornerRadius::same(24))
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        if self.page == 0 {
                            common::page(ui, ICON_WIDGETS, "Widgets", "Material 3 building blocks shared by every app.", |ui| {
                                card(ui, "Controls", |ui| {
                                    row(ui, "Switch", "A setting that's on or off.", |ui| {
                                        widgets::switch(ui, &mut self.on);
                                    });
                                    ui.separator();
                                    row(ui, "Segmented button", "One of a few choices.", |ui| {
                                        widgets::segmented(ui, &mut self.view, &[(View::Grid, ICON_GRID_VIEW, "Grid"), (View::List, ICON_VIEW_LIST, "List")]);
                                    });
                                    ui.separator();
                                    row(ui, "Buttons", "Filled, tonal, outlined, text.", |ui| {
                                        widgets::button(ui, ButtonKind::Text, None, "Text");
                                        widgets::button(ui, ButtonKind::Outlined, None, "Outlined");
                                        widgets::button(ui, ButtonKind::Tonal, Some(ICON_DOWNLOAD), "Tonal");
                                        if widgets::button(ui, ButtonKind::Filled, Some(ICON_CHECK), "Filled").clicked() {
                                            self.toasts.success("Saved");
                                        }
                                    });
                                    ui.separator();
                                    row(ui, "Chips and progress", "", |ui| {
                                        widgets::linear_progress(ui, Some(0.6), 160.0);
                                        if widgets::filter_chip(ui, "Installed", self.chip, None).clicked() {
                                            self.chip = !self.chip;
                                        }
                                    });
                                });
                                common::banner(ui, ICON_INFO, "Banners carry status and warnings, with optional actions.", false, |ui| {
                                    widgets::button(ui, ButtonKind::Filled, None, "Action");
                                });
                                card(ui, "Lists", |ui| {
                                    common::list_row(ui, ICON_WIFI, true, "Home Net", "Connected", |_| {});
                                    common::list_row(ui, ICON_BLUETOOTH, false, "Headphones", "Paired", |_| {});
                                });
                            });
                        } else {
                            common::page(ui, ICON_SETTINGS, "Appearance", "The shared Appearance card.", |ui| {
                                if prefs::appearance_card(ui, &mut self.settings, &self.theme) {
                                    self.theme.apply(&self.settings);
                                }
                            });
                        }
                    });
            });
        let ctx = ui.ctx().clone();
        self.toasts.show(&ctx, &p, 20.0);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.theme.palette().surface.to_normalized_gamma_f32()
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "material-3-ui gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            let settings = ThemeSettings::default();
            let theme = ThemeManager::new(&cc.egui_ctx, &settings);
            Ok(Box::new(Gallery {
                settings,
                theme,
                toasts: Toasts::default(),
                page: 0,
                on: true,
                view: View::Grid,
                query: String::new(),
                chip: true,
            }))
        }),
    )
}
