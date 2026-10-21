//! # material-3-ui
//!
//! The shared look and plumbing of the Material 3 apps (Settings, Files,
//! Music, Games, Browser, AI, Viewer, Text, Monitor, Screenshots, Calendar):
//! a Material You palette that follows end4-pC's colors live, the egui style
//! and fonts built from it, M3 widgets, page scaffolding, toasts, the shared
//! Appearance preferences card, and small system helpers.
//!
//! ```no_run
//! use material_3_ui::theme::{ThemeManager, ThemeSettings};
//! # fn setup(ctx: &egui::Context) {
//! let settings = ThemeSettings::default();
//! let mut theme = ThemeManager::new(ctx, &settings);
//! // every frame:
//! theme.poll(&settings);
//! # }
//! ```

pub mod common;
pub mod prefs;
pub mod sys;
pub mod theme;
pub mod toasts;
pub mod widgets;

pub use egui;
pub use egui_material_icons as icons;
