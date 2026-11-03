use std::fmt::Formatter;

use gtk::pango;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

use crate::toml_io::get_config_file_path;

static DEFAULT_THEME: &str = include_str!("../assets/theme.toml");

/// An RGBA color, with each channel in `[0.0, 1.0]`.
///
/// (De)serialized as `"#rrggbb"` or `"#rrggbbaa"`.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Default for Color {
    fn default() -> Self {
        Self::rgba(1.0, 1.0, 1.0, 1.0)
    }
}

impl Color {
    pub const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub fn with_alpha(self, a: f64) -> Self {
        Self { a, ..self }
    }

    /// Equivalent of the terminal's "dim" (aka "faint") attribute.
    pub fn dimmed(self) -> Self {
        Self {
            a: self.a * 0.55,
            ..self
        }
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let hex = s.trim().strip_prefix('#')?;
        if !hex.is_ascii() || (hex.len() != 6 && hex.len() != 8) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
        Some(Self {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a: if hex.len() == 8 { channel(6)? } else { 1.0 },
        })
    }

    pub fn to_hex(self) -> String {
        let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", c(self.r), c(self.g), c(self.b))
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", c(self.r), c(self.g), c(self.b), c(self.a))
        }
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Color::from_hex(&s)
            .ok_or_else(|| D::Error::custom(format!("invalid color {s:?}, expected \"#rrggbb\" or \"#rrggbbaa\"")))
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Theme {
    pub list_header: Color,
    pub list_entry: Color,
    pub list_decoration: Color,
    pub list_left_line: Color,
    pub caret: Color,
    pub background: Color,
    pub background_selected: Color,
    pub background_opacity: f64,
    pub font: String,
    pub line_height: f64,
    pub window_decorations: bool,
}

impl Default for Theme {
    fn default() -> Self {
        toml::from_str(DEFAULT_THEME).expect("assets/theme.toml should be a valid theme")
    }
}

impl Theme {
    /// Loads `~/.config/vaporeon/theme.toml`, falling back to the default theme for any missing value
    /// (or for everything, if the file doesn't exist or is invalid).
    pub fn load() -> Self {
        let Ok(path) = get_config_file_path("theme") else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(user_theme) => Self::from_str_with_defaults(&user_theme).unwrap_or_else(|err| {
                log::error!("Error in {path:?}. Will use the default theme. Error was:\n{err}");
                Self::default()
            }),
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::error!("Could not read {path:?}: {err:?}");
                }
                Self::default()
            }
        }
    }

    fn from_str_with_defaults(user_theme: &str) -> Result<Self, toml::de::Error> {
        let mut table: toml::Table = toml::from_str(DEFAULT_THEME)?;
        let user_table: toml::Table = toml::from_str(user_theme)?;
        table.extend(user_table);
        Theme::deserialize(table)
    }

    /// The background color, with `background_opacity` applied.
    pub fn window_background(&self) -> Color {
        let opacity = self.background_opacity.clamp(0.0, 1.0);
        self.background.with_alpha(self.background.a * opacity)
    }

    pub fn font_description(&self) -> pango::FontDescription {
        pango::FontDescription::from_string(&self.font)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_parses() {
        let theme = Theme::default();
        assert_eq!(theme.list_header, Color::from_hex("#d79921").unwrap());
        assert!(!theme.window_decorations);
    }

    #[test]
    fn user_theme_overrides_defaults() {
        let theme = Theme::from_str_with_defaults("background_opacity = 0.0\nlist_entry = \"#ff000080\"").unwrap();
        assert_eq!(theme.background_opacity, 0.0);
        assert_eq!(theme.list_entry, Color::rgba(1.0, 0.0, 0.0, 128.0 / 255.0));
        assert_eq!(theme.font, Theme::default().font);
    }

    #[test]
    fn color_hex_round_trip() {
        for s in ["#d79921", "#28282880"] {
            assert_eq!(Color::from_hex(s).unwrap().to_hex(), s);
        }
        assert!(Color::from_hex("d79921").is_none());
        assert!(Color::from_hex("#d7992").is_none());
    }
}
