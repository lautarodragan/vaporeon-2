//! A bar at the bottom of the window, for messages and questions.
//!
//! - Messages (e.g. "Saved", errors) disappear after a few seconds.
//! - Prompts show a question with options, each chosen with a key or a click. They stay until one is chosen.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gtk::{glib, prelude::*};

/// How long messages are shown.
const MESSAGE_DURATION: Duration = Duration::from_secs(4);
const ERROR_DURATION: Duration = Duration::from_secs(10);

/// An option of a prompt: the key that chooses it, and its description.
pub type PromptOption = (char, &'static str);

pub struct StatusBar {
    pub widget: gtk::Box,
    text: gtk::Label,
    options: gtk::Box,
    hide_timer: Rc<RefCell<Option<glib::SourceId>>>,
    prompt_active: Cell<bool>,
}

impl StatusBar {
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .visible(false)
            .build();
        widget.add_css_class("row");
        widget.add_css_class("status-bar");

        let text = gtk::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let options = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        options.add_css_class("options");

        widget.append(&text);
        widget.append(&options);

        Self {
            widget,
            text,
            options,
            hide_timer: Rc::new(RefCell::new(None)),
            prompt_active: Cell::new(false),
        }
    }

    /// Whether a prompt is waiting for an answer.
    pub fn is_prompt_active(&self) -> bool {
        self.prompt_active.get()
    }

    /// Shows a message for a few seconds. Ignored while a prompt is shown (it's logged anyway).
    pub fn show_message(&self, text: &str, is_error: bool) {
        if self.is_prompt_active() {
            return;
        }
        self.cancel_hide_timer();
        self.clear_options();
        self.set_text(text, is_error);
        self.widget.set_visible(true);

        let duration = if is_error { ERROR_DURATION } else { MESSAGE_DURATION };
        let id = glib::timeout_add_local_once(duration, {
            let widget = self.widget.downgrade();
            let hide_timer = self.hide_timer.clone();
            move || {
                hide_timer.borrow_mut().take();
                if let Some(widget) = widget.upgrade() {
                    widget.set_visible(false);
                }
            }
        });
        *self.hide_timer.borrow_mut() = Some(id);
    }

    /// Shows a question, with options. `on_click` is called with the key of the option clicked.
    /// Keys are handled by the caller (see [`StatusBar::is_prompt_active`]), which calls [`StatusBar::hide`] once
    /// an option is chosen.
    pub fn show_prompt(&self, text: &str, options: &[PromptOption], on_click: Rc<dyn Fn(char)>) {
        self.cancel_hide_timer();
        self.clear_options();
        self.set_text(text, true);

        for &(key, description) in options {
            let option = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            option.add_css_class("option");

            let key_label = gtk::Label::new(Some(&format!("[{key}]")));
            key_label.add_css_class("key");
            let description_label = gtk::Label::new(Some(description));

            option.append(&key_label);
            option.append(&description_label);

            let click = gtk::GestureClick::new();
            click.connect_pressed({
                let on_click = on_click.clone();
                move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    on_click(key);
                }
            });
            option.add_controller(click);
            option.set_cursor_from_name(Some("pointer"));

            self.options.append(&option);
        }

        self.prompt_active.set(true);
        self.widget.set_visible(true);
    }

    pub fn hide(&self) {
        self.cancel_hide_timer();
        self.prompt_active.set(false);
        self.clear_options();
        self.widget.set_visible(false);
    }

    fn set_text(&self, text: &str, highlight: bool) {
        self.text.set_text(text);
        if highlight {
            self.text.add_css_class("header");
        } else {
            self.text.remove_css_class("header");
        }
    }

    fn clear_options(&self) {
        while let Some(child) = self.options.first_child() {
            self.options.remove(&child);
        }
    }

    fn cancel_hide_timer(&self) {
        if let Some(id) = self.hide_timer.borrow_mut().take() {
            id.remove();
        }
    }
}
