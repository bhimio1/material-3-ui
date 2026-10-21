//! Every widget in one window: `cargo run --example gallery --all-features`
//! (the Markdown and mini-month demos need their features). A page name as
//! the argument opens that page: `widgets`, `tiles`, `dashboard`,
//! `content` or `appearance`.

use material_3_ui::charts::{self, Series};
use material_3_ui::common::{self, card, row, TileFn};
use material_3_ui::icons::icons::*;
use material_3_ui::prefs;
use material_3_ui::theme::{ThemeManager, ThemeSettings};
use material_3_ui::thumbs::Thumbnails;
use material_3_ui::toasts::Toasts;
use material_3_ui::widgets::{self, ButtonKind};
use material_3_ui::{format, keys};
use std::path::Path;
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, PartialEq)]
enum View {
    Grid,
    List,
}

const PAGES: [(&str, &str); 5] = [
    ("widgets", "Widgets"),
    ("tiles", "Tiles"),
    ("dashboard", "Dashboard"),
    ("content", "Content"),
    ("appearance", "Appearance"),
];

const MARKDOWN: &str = "## Markdown\n\nChat replies and previews render **bold**, *italic*, `code`, ~~struck~~ and [links](https://github.com/bhimio1/material-3-ui).\n\n- [x] Lists, task lists and quotes\n- [ ] Tables that scroll when wide\n\n```rust\nfn main() {\n    let greeting = \"hello\"; // colored from the palette\n    println!(\"{greeting}, {}\", 42);\n}\n```\n";

struct Gallery {
    settings: ThemeSettings,
    theme: ThemeManager,
    toasts: Toasts<()>,
    thumbs: Thumbnails,
    page: usize,
    on: bool,
    view: View,
    query: String,
    name: String,
    chip: bool,
    color: usize,
    wrap: bool,
    #[cfg(feature = "calendar")]
    month: chrono::NaiveDate,
    #[cfg(feature = "calendar")]
    day: chrono::NaiveDate,
}

/// A made-up load curve, so the charts look alive.
fn wave(t: f64, phase: f64, base: f32, swing: f32) -> f32 {
    let v = (t * 0.7 + phase).sin() * 0.6 + (t * 2.3 + phase * 2.0).sin() * 0.3 + (t * 5.1).sin() * 0.1;
    (base + swing * v as f32).clamp(0.0, 1.0)
}

fn history(now: f64, phase: f64, base: f32, swing: f32) -> Vec<f32> {
    (0..60).map(|i| wave(now - (59 - i) as f64 * 0.5, phase, base, swing)).collect()
}

/// A stand-in for decoding a picture: a gradient from the file name.
fn fake_thumbnail(path: &Path) -> Option<egui::ColorImage> {
    std::thread::sleep(Duration::from_millis(30));
    let seed = path.to_string_lossy().bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let (w, h) = (192, 120);
    let hue = |k: u32| ((seed >> k) & 0xff) as f32 / 255.0;
    let pixels = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let r = (60.0 + 150.0 * (hue(0) * (1.0 - x) + x * y)) as u8;
            let g = (50.0 + 140.0 * (hue(8) * y + (1.0 - y) * 0.4)) as u8;
            let b = (90.0 + 150.0 * (hue(16) * x + 0.3 * (1.0 - x * y))) as u8;
            egui::Color32::from_rgb(r, g, b)
        })
        .collect();
    Some(egui::ColorImage { size: [w, h], pixels, source_size: egui::vec2(w as f32, h as f32) })
}

impl Gallery {
    fn widgets_page(&mut self, ui: &mut egui::Ui) {
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
    }

    fn tiles_page(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette().clone();
        common::page(ui, ICON_DASHBOARD, "Tiles", "Big actions, thumbnails, swatches, fields and menus.", |ui| {
            ui.add_space(16.0);
            let gap = 12.0;
            let w = ((ui.available_width() - gap * 2.0) / 3.0).floor();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                if widgets::action_tile(ui, ICON_SCREENSHOT_REGION, "Region", "Drag to select", w, true).clicked() {
                    self.toasts.info("Region");
                }
                widgets::action_tile(ui, ICON_WEB_ASSET, "Window", "Click a window", w, false);
                widgets::action_tile(ui, ICON_MONITOR, "Screen", "The focused output", w, false);
            });
            ui.add_space(gap);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                let shots = [
                    ("Screenshot_20261021_101530.png", 412_000u64, ICON_IMAGE, None),
                    ("Screenshot_20261021_094212.png", 1_830_000, ICON_IMAGE, Some(ICON_CONTENT_COPY)),
                    ("Recording_20261020_183005.mp4", 24_600_000, ICON_MOVIE, Some(ICON_PLAY_ARROW)),
                ];
                for (name, size, fallback, badge) in shots {
                    let tex = self.thumbs.get(Path::new(name), SystemTime::UNIX_EPOCH);
                    widgets::thumb_tile(ui, tex.as_ref(), fallback, name, &format!("Oct 21 · {}", format::file_size(size)), w, badge);
                }
            });
            card(ui, "Pieces", |ui| {
                row(ui, "Color swatches", "Pick a pen color.", |ui| {
                    let colors = [p.error, p.tertiary, p.primary, egui::Color32::from_rgb(0x4C, 0xAF, 0x50), egui::Color32::WHITE];
                    for (i, c) in colors.into_iter().enumerate().rev() {
                        if widgets::color_swatch(ui, c, self.color == i, "").clicked() {
                            self.color = i;
                        }
                    }
                });
                ui.separator();
                row(ui, "Field and status chips", "A rounded text field; compact status-bar buttons.", |ui| {
                    widgets::chip(ui, "UTF-8", "Encoding");
                    widgets::chip(ui, "Ln 12, Col 4", "Go to line");
                    widgets::field(ui, &mut self.name, "File name", 180.0, egui::Id::unique("gallery-field"));
                });
                ui.separator();
                // A menu doesn't fit a one-line row: title on the left, the
                // menu (as a context menu draws it) on the right.
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Menu items").size(14.5).color(p.on_surface));
                        ui.label(egui::RichText::new("Icons, a check mark and shortcut hints.").size(12.5).color(p.on_surface_variant));
                    });
                    ui.add_space((ui.available_width() - 238.0).max(0.0));
                    egui::Frame::new()
                        .fill(p.surface_container_high)
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(230.0);
                                widgets::menu_item(ui, Some(ICON_SAVE), "Save", "Ctrl+S", false);
                                if widgets::menu_item(ui, None, "Word wrap", "Alt+Z", self.wrap) {
                                    self.wrap = !self.wrap;
                                }
                            });
                        });
                });
                ui.add_space(8.0);
                ui.separator();
                row(ui, "File icons", "By name and extension.", |ui| {
                    for name in ["notes.txt", "photo.png", "song.flac", "main.rs", "PKGBUILD"].iter().rev() {
                        widgets::icon(ui, widgets::file_icon(name, false, false), 22.0, p.on_surface_variant).on_hover_text(*name);
                    }
                });
            });
        });
    }

    fn dashboard_page(&mut self, ui: &mut egui::Ui) {
        let p = self.theme.palette().clone();
        let now = ui.input(|i| i.time);
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        let cpu = history(now, 0.0, 0.35, 0.3);
        let cores: Vec<f32> = (0..16).map(|i| wave(now, i as f64 * 1.7, 0.4, 0.55)).collect();
        let down: Vec<f32> = history(now, 2.0, 0.45, 0.4).iter().map(|v| v * 4.0e6).collect();
        let up: Vec<f32> = history(now, 4.0, 0.2, 0.2).iter().map(|v| v * 4.0e6).collect();
        let (mem, swap) = (wave(now, 1.0, 0.62, 0.05), 0.93);
        common::wide_page(ui, ICON_MONITORING, "Dashboard", "Charts, meters and a responsive tile grid.", 1200.0, |ui| {
            let tiles: Vec<TileFn> = vec![
                Box::new(|ui| {
                    common::tile(ui, ICON_MEMORY, "Processor", &format::percent(cpu[59]), |ui| {
                        charts::area_chart(ui, egui::vec2(ui.available_width(), 96.0), &[Series { values: &cpu, color: p.primary }], 1.0, 60);
                        ui.add_space(8.0);
                        charts::bars(ui, &cores, 40.0);
                        ui.add_space(6.0);
                        common::stat_line(ui, "Frequency", &format::mhz(3412.0));
                        common::stat_line(ui, "Uptime", &format::duration(2.0 * 86400.0 + 4.5 * 3600.0));
                    });
                }),
                Box::new(|ui| {
                    common::tile(ui, ICON_SWAP_VERT, "Network", &format::rate(down[59] as f64), |ui| {
                        let max = charts::nice_max(down.iter().chain(&up).fold(0.0, |a, b| a.max(*b)));
                        let series = [Series { values: &down, color: p.primary }, Series { values: &up, color: p.tertiary }];
                        charts::area_chart(ui, egui::vec2(ui.available_width(), 96.0), &series, max, 60);
                        ui.horizontal(|ui| {
                            charts::legend(ui, p.primary, &format!("Down {}", format::rate(down[59] as f64)));
                            ui.add_space(8.0);
                            charts::legend(ui, p.tertiary, &format!("Up {}", format::rate(up[59] as f64)));
                        });
                    });
                }),
                Box::new(|ui| {
                    common::tile(ui, ICON_STORAGE, "Memory", &format::bytes((mem * 32.0 * 1024.0 * 1024.0 * 1024.0) as u64), |ui| {
                        common::stat_line(ui, "RAM", &format::percent(mem));
                        charts::meter(ui, mem, ui.available_width());
                        ui.add_space(8.0);
                        common::stat_line(ui, "Swap", &format::percent(swap));
                        charts::meter(ui, swap, ui.available_width());
                        ui.add_space(8.0);
                        common::stat_line(ui, "Disk", &format!("{} free", format::bytes(412 * 1024 * 1024 * 1024)));
                    });
                }),
            ];
            common::tile_grid(ui, 300.0, tiles);
        });
    }

    fn content_page(&mut self, ui: &mut egui::Ui) {
        common::page(ui, ICON_ARTICLE, "Content", "Markdown, code and the mini month.", |ui| {
            ui.add_space(8.0);
            let width = ui.available_width();
            ui.horizontal_top(|ui| {
                let side = 260.0;
                let down = egui::Layout::top_down(egui::Align::Min);
                ui.allocate_ui_with_layout(egui::vec2(width - side - 16.0, 0.0), down, |ui| {
                    card(ui, "", |ui| {
                        #[cfg(feature = "markdown")]
                        material_3_ui::markdown::show(ui, MARKDOWN, 14.0, &mut |url| ui_open(&url));
                        #[cfg(not(feature = "markdown"))]
                        {
                            let p = widgets::palette(ui);
                            ui.label(egui::RichText::new(MARKDOWN.lines().nth(2).unwrap_or_default()).color(p.on_surface));
                            ui.add(egui::Label::new(material_3_ui::code::highlight("fn main() { let x = \"hi\"; } // code", "rust", &p, 13.0)));
                        }
                    });
                });
                ui.add_space(16.0);
                ui.allocate_ui_with_layout(egui::vec2(side, 0.0), down, |ui| {
                    card(ui, "", |ui| {
                        #[cfg(feature = "calendar")]
                        {
                            ui.add_space(8.0);
                            use chrono::Datelike;
                            if let Some(d) = material_3_ui::calendar::mini_month(ui, &mut self.month, self.day, chrono::Weekday::Mon, |d| d.day() % 5 == 2) {
                                self.day = d;
                            }
                            ui.add_space(4.0);
                        }
                        #[cfg(not(feature = "calendar"))]
                        common::note(ui, "Build with --features calendar for the mini month.");
                    });
                });
            });
        });
    }
}

#[cfg(feature = "markdown")]
fn ui_open(url: &str) {
    let _ = material_3_ui::sys::cmd::spawn_detached("xdg-open", &[url]);
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.theme.poll(&self.settings);
        let p = self.theme.palette().clone();
        // Ctrl+PageDown / Ctrl+PageUp switch pages; exact modifiers only.
        if keys::take_key(ui.ctx(), egui::Modifiers::COMMAND, egui::Key::PageDown) {
            self.page = (self.page + 1) % PAGES.len();
        }
        if keys::take_key(ui.ctx(), egui::Modifiers::COMMAND, egui::Key::PageUp) {
            self.page = (self.page + PAGES.len() - 1) % PAGES.len();
        }
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
                let icons = [ICON_WIDGETS, ICON_DASHBOARD, ICON_MONITORING, ICON_ARTICLE, ICON_SETTINGS];
                for (i, (icon, (_, name))) in icons.into_iter().zip(PAGES).enumerate() {
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
                        match self.page {
                            0 => self.widgets_page(ui),
                            1 => self.tiles_page(ui),
                            2 => self.dashboard_page(ui),
                            3 => self.content_page(ui),
                            _ => common::page(ui, ICON_SETTINGS, "Appearance", "The shared Appearance card.", |ui| {
                                if prefs::appearance_card(ui, &mut self.settings, &self.theme) {
                                    self.theme.apply(&self.settings);
                                }
                            }),
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
    let page = std::env::args().nth(1).and_then(|a| PAGES.iter().position(|(key, _)| *key == a)).unwrap_or(0);
    eframe::run_native(
        "material-3-ui gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let settings = ThemeSettings::default();
            let theme = ThemeManager::new(&cc.egui_ctx, &settings);
            #[cfg(feature = "calendar")]
            let today = chrono::Local::now().date_naive();
            Ok(Box::new(Gallery {
                settings,
                theme,
                toasts: Toasts::default(),
                thumbs: Thumbnails::new(&cc.egui_ctx, fake_thumbnail),
                page,
                on: true,
                view: View::Grid,
                query: String::new(),
                name: "notes.md".into(),
                chip: true,
                color: 2,
                wrap: true,
                #[cfg(feature = "calendar")]
                month: today,
                #[cfg(feature = "calendar")]
                day: today,
            }))
        }),
    )
}
