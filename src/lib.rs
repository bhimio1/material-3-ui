//! # material-3-ui
//!
//! The shared look and plumbing of the Material 3 apps (Settings, Files,
//! Music, Games, Browser, AI, Viewer, Text, Monitor, Screenshots, Calendar):
//! a Material You palette that follows end4-pC's colors live, the egui style
//! and fonts built from it, M3 widgets, charts, page scaffolding, toasts, the
//! shared Appearance preferences card, and small system helpers.
//!
//! Heavier pieces are behind cargo features: `notify` (desktop
//! notifications), `http` and `oauth` (browser sign-in), `markdown`,
//! `calendar` (the mini month), `backups`, `image` (thumbnail decoding) and
//! `testing` (a mock HTTP server).
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

pub mod charts;
pub mod code;
pub mod common;
pub mod format;
pub mod fs;
pub mod keys;
pub mod prefs;
pub mod secrets;
pub mod sys;
pub mod theme;
pub mod thumbs;
pub mod toasts;
pub mod widgets;

#[cfg(feature = "calendar")]
pub mod calendar;
#[cfg(feature = "http")]
pub mod http;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "notify")]
pub mod notify;
#[cfg(feature = "oauth")]
pub mod oauth;
#[cfg(feature = "testing")]
pub mod testing;

pub use egui;
pub use egui_material_icons as icons;
