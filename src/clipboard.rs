use gtk::{gdk, prelude::*};

/// Puts `text` in the system clipboard.
pub fn copy(text: &str) {
    match gdk::Display::default() {
        Some(display) => display.clipboard().set_text(text),
        None => log::error!("Clipboard error: no display"),
    }
}
