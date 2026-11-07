//! The details panel: the selected task's title, and its description.
//!
//! The description is always shown in a [`gtk::TextView`], which only becomes editable while editing,
//! so it looks exactly the same whether it's being edited or not.

use std::{cell::Cell, rc::Rc};

use gtk::{glib, prelude::*};

use crate::{
    canvas::Metrics,
    components::task_details::TaskDetails,
    view::{fade_edges::FadeEdges, task_list_view::OnEdit},
};

pub struct DetailsView {
    /// The scrolled details, with their edges faded out.
    pub widget: FadeEdges,
    title: gtk::Label,
    text_view: gtk::TextView,
    is_editing: Rc<Cell<bool>>,
    syncing: Rc<Cell<bool>>,
}

impl DetailsView {
    /// `on_click` is called when the description is clicked while not editing it.
    pub fn new(on_edit: OnEdit, on_click: Rc<dyn Fn()>, syncing: Rc<Cell<bool>>) -> Self {
        let title = gtk::Label::builder()
            .xalign(0.0)
            .single_line_mode(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        title.add_css_class("row");
        title.add_css_class("header");

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.add_css_class("row");

        let text_view = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::None)
            .accepts_tab(false)
            .editable(false)
            .cursor_visible(false)
            .focusable(false)
            .vexpand(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&title);
        content.append(&spacer);
        content.append(&text_view);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&content)
            .build();
        super::scrollbars::hide(&scrolled);

        let is_editing = Rc::new(Cell::new(false));

        text_view.buffer().connect_changed(glib::clone!(
            #[strong]
            syncing,
            move |buffer| {
                if !syncing.get() {
                    on_edit(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
                }
            }
        ));

        // Capture phase, so we see the click before the text view does.
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        click.connect_pressed(glib::clone!(
            #[strong]
            is_editing,
            move |gesture, _, _, _| {
                if !is_editing.get() {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    on_click();
                }
            }
        ));
        text_view.add_controller(click);

        Self {
            widget: FadeEdges::new(&scrolled),
            title,
            text_view,
            is_editing,
            syncing,
        }
    }

    pub fn update(&mut self, details: &TaskDetails, has_focus: bool, metrics: &Metrics) {
        self.widget.set_size(metrics.line_height);
        if self.title.text() != details.title() {
            self.title.set_text(details.title());
        }
        set_class(&self.title, "dim", !has_focus);
        set_class(&self.text_view, "dim", !has_focus);

        // Same line spacing as the task lists.
        let offset = metrics.text_offset();
        self.text_view.set_pixels_above_lines(offset as i32);
        self.text_view
            .set_pixels_below_lines((metrics.line_height - metrics.text_height - offset) as i32);

        let buffer = self.text_view.buffer();

        if details.is_editing() {
            if !self.is_editing.get() {
                self.is_editing.set(true);
                self.set_text(details.description());
                self.text_view.set_editable(true);
                self.text_view.set_cursor_visible(true);
                self.text_view.set_focusable(true);
                self.text_view.grab_focus();
                buffer.place_cursor(&buffer.end_iter());
            }
            return;
        }

        if self.is_editing.get() {
            self.is_editing.set(false);
            self.text_view.set_editable(false);
            self.text_view.set_cursor_visible(false);
            self.text_view.set_focusable(false);
        }

        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != details.description() {
            self.set_text(details.description());
        }
    }

    fn set_text(&self, text: &str) {
        self.syncing.set(true);
        self.text_view.buffer().set_text(text);
        self.syncing.set(false);
    }
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, enabled: bool) {
    if enabled {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}
