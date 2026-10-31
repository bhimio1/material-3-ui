//! The shell's resolved look, exported by Material 3 Settings' shell
//! integration to `$XDG_STATE_HOME/m3/style.json`.
//!
//! Every supported shell (end-4's ii, pctrade's end4-pC, iNiR, Dux Shell)
//! gets a small exporter that binds to the shell's own theme object and
//! rewrites this file whenever it changes. So the apps follow presets,
//! custom themes and light/dark switches without knowing about any of them.
//!
//! ```json
//! {
//!   "version": 1,
//!   "shell": "ii",
//!   "dark": true,
//!   "colors": { "primary": "#d0bcff", "on_primary": "#381e72", "…": "…" },
//!   "fonts": { "main": "Google Sans Flex", "title": "…", "monospace": "…" },
//!   "radius": { "small": 12, "normal": 17, "large": 23 },
//!   "transparency": { "background": 0.0, "content": 0.0 },
//!   "wallpaper": "/home/me/Pictures/Wallpapers/x.jpg",
//!   "wallpaper_thumbnail": ""
//! }
//! ```
//!
//! Color keys are the snake_case Material 3 role names, the same keys as a
//! matugen `theme.json`; roles a shell doesn't have are filled in from its
//! primary color.

use super::palette::M3Palette;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const STYLE_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Fonts {
    pub main: String,
    pub title: String,
    pub monospace: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Radius {
    pub small: f32,
    pub normal: f32,
    pub large: f32,
}

impl Default for Radius {
    /// Material 3's own scale (and ii's defaults).
    fn default() -> Self {
        Self { small: 12.0, normal: 17.0, large: 23.0 }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Transparency {
    /// 0 = opaque.
    pub background: f32,
    pub content: f32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ShellStyle {
    pub version: u32,
    /// Which shell wrote it (`ii`, `end4-pC`, `inir`, `dux`).
    pub shell: String,
    pub dark: Option<bool>,
    pub colors: HashMap<String, String>,
    pub fonts: Fonts,
    pub radius: Radius,
    pub transparency: Transparency,
    /// The current wallpaper's path (empty when unknown or a solid color);
    /// Settings' login screen and boot animation use it.
    pub wallpaper: String,
    /// A still frame, for video wallpapers.
    pub wallpaper_thumbnail: String,
}

/// `$XDG_STATE_HOME/m3/style.json`.
pub fn style_path() -> Option<PathBuf> {
    dirs::state_dir().map(|d| d.join("m3/style.json"))
}

impl ShellStyle {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let s: ShellStyle = serde_json::from_str(text)?;
        if s.version > STYLE_VERSION {
            anyhow::bail!("style.json version {} is newer than this app understands ({STYLE_VERSION})", s.version);
        }
        if !s.colors.contains_key("primary") {
            anyhow::bail!("style.json has no primary color");
        }
        Ok(s)
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    pub fn palette(&self) -> M3Palette {
        M3Palette::from_color_map(&self.colors, self.dark)
    }

    /// How much rounder (or sharper) the shell is than Material 3's default,
    /// to scale the app's own corner radii by.
    pub fn radius_scale(&self) -> f32 {
        (self.radius.normal / Radius::default().normal).clamp(0.0, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const II: &str = r##"{
        "version": 1, "shell": "ii", "dark": true,
        "colors": {"primary": "#d0bcff", "background": "#141218", "surface_container_low": "#1d1b20"},
        "fonts": {"main": "Google Sans Flex", "title": "Google Sans Flex", "monospace": "JetBrains Mono NF"},
        "radius": {"small": 6, "normal": 8.5, "large": 11.5},
        "transparency": {"background": 0.1, "content": 0.5}
    }"##;

    #[test]
    fn parses_an_exported_style() {
        let s = ShellStyle::parse(II).unwrap();
        assert_eq!(s.shell, "ii");
        assert_eq!(s.fonts.monospace, "JetBrains Mono NF");
        assert!((s.radius_scale() - 0.5).abs() < 1e-3);
        let p = s.palette();
        assert_eq!(super::super::palette::to_hex(p.primary).to_lowercase(), "#d0bcff");
        assert_eq!(super::super::palette::to_hex(p.surface_container_low).to_lowercase(), "#1d1b20");
        assert!(p.is_dark());
    }

    #[test]
    fn light_mode_follows_the_flag() {
        let s = ShellStyle::parse(r##"{"version":1,"dark":false,"colors":{"primary":"#6750a4"}}"##).unwrap();
        assert!(!s.palette().is_dark());
        assert_eq!(s.radius, Radius::default());
    }

    #[test]
    fn rejects_newer_versions_and_missing_primary() {
        assert!(ShellStyle::parse(r##"{"version":2,"colors":{"primary":"#fff"}}"##).is_err());
        assert!(ShellStyle::parse(r##"{"version":1,"colors":{}}"##).is_err());
    }
}
