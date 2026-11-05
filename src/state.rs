//! Things worth remembering between runs, but not worth backing up: `$XDG_STATE_HOME/vaporeon/state.toml`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::toml_io::get_state_dir_path;

/// Smallest window size we restore, in case the saved one is broken (e.g. edited by hand).
const MIN_WIDTH: i32 = 300;
const MIN_HEIGHT: i32 = 200;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(default)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            maximized: false,
        }
    }
}

impl WindowState {
    fn sanitized(self) -> Self {
        Self {
            width: self.width.max(MIN_WIDTH),
            height: self.height.max(MIN_HEIGHT),
            ..self
        }
    }
}

/// The zoom levels Ctrl+ and Ctrl- go through, like browsers do.
pub const ZOOM_LEVELS: [f64; 13] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(default)]
pub struct State {
    pub window: WindowState,
    /// How big everything is (fonts, spacing...), 1.0 being the size set in the theme.
    pub zoom: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            window: WindowState::default(),
            zoom: 1.0,
        }
    }
}

/// The next zoom level, bigger (`direction` > 0) or smaller (< 0) than `zoom`.
pub fn next_zoom(zoom: f64, direction: i32) -> f64 {
    let next = if direction > 0 {
        ZOOM_LEVELS.iter().find(|&&level| level > zoom + 0.001)
    } else {
        ZOOM_LEVELS.iter().rev().find(|&&level| level < zoom - 0.001)
    };
    next.copied().unwrap_or(zoom)
}

fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() {
        zoom.clamp(ZOOM_LEVELS[0], ZOOM_LEVELS[ZOOM_LEVELS.len() - 1])
    } else {
        1.0
    }
}

impl State {
    fn path() -> Option<PathBuf> {
        get_state_dir_path().map(|dir| dir.join("state.toml"))
    }

    /// Loads the state. If there's none (or it's broken), returns the default one: losing it isn't a big deal.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::from_str(&text).unwrap_or_else(|err| {
                log::warn!("Ignoring {path:?}, it's not valid: {err}");
                Self::default()
            }),
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Can't read {path:?}: {err}");
                }
                Self::default()
            }
        }
    }

    fn from_str(text: &str) -> Result<Self, toml::de::Error> {
        let state: State = toml::from_str(text)?;
        Ok(State {
            window: state.window.sanitized(),
            zoom: clamp_zoom(state.zoom),
        })
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        let result = toml::to_string_pretty(self)
            .map_err(|err| err.to_string())
            .and_then(|text| {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
                }
                std::fs::write(&path, text).map_err(|err| err.to_string())
            });
        if let Err(err) = result {
            log::warn!("Can't save {path:?}: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let state = State {
            window: WindowState {
                width: 900,
                height: 600,
                maximized: true,
            },
            zoom: 1.25,
        };
        assert_eq!(State::from_str(&toml::to_string(&state).unwrap()).unwrap(), state);
    }

    #[test]
    fn zoom_levels() {
        assert_eq!(next_zoom(1.0, 1), 1.1);
        assert_eq!(next_zoom(1.0, -1), 0.9);
        assert_eq!(next_zoom(3.0, 1), 3.0);
        assert_eq!(next_zoom(0.5, -1), 0.5);
        // In between levels (e.g. edited by hand): goes to the nearest one in that direction.
        assert_eq!(next_zoom(1.3, 1), 1.5);
        assert_eq!(next_zoom(1.3, -1), 1.25);
        assert_eq!(State::from_str("zoom = 99").unwrap().zoom, 3.0);
    }

    #[test]
    fn missing_values_use_defaults_and_tiny_sizes_are_ignored() {
        assert_eq!(State::from_str("").unwrap(), State::default());
        let state = State::from_str("[window]\nwidth = 10\nheight = -5\n").unwrap();
        assert_eq!(state.window.width, MIN_WIDTH);
        assert_eq!(state.window.height, MIN_HEIGHT);
        assert!(!state.window.maximized);
    }
}
