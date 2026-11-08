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
    /// The title, and under it the scrolled description.
    pub widget: gtk::Box,
    /// The scrolled description, with its edges faded out.
    fade: FadeEdges,
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

        // The text view goes right into the scrolled window (it's scrollable itself), so it scrolls with the
        // scrolled window's adjustments, e.g. to follow the cursor sideways. Inside a box, it would scroll on its
        // own, and the scrolled window wouldn't know there's more to scroll to.
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&text_view)
            .build();
        super::scrollbars::hide(&scrolled);
        let fade = FadeEdges::new(&scrolled);

        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.append(&title);
        widget.append(&spacer);
        widget.append(&fade);

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
            widget,
            fade,
            title,
            text_view,
            is_editing,
            syncing,
        }
    }

    /// With `wrap`, long lines wrap instead of scrolling sideways.
    pub fn update(&mut self, details: &TaskDetails, has_focus: bool, metrics: &Metrics, wrap: bool) {
        self.fade.set_size(metrics.line_height);
        if self.title.text() != details.title() {
            self.title.set_text(details.title());
        }
        set_class(&self.title, "dim", !has_focus);
        set_class(&self.text_view, "dim", !has_focus);

        self.text_view.set_wrap_mode(if wrap {
            gtk::WrapMode::WordChar
        } else {
            gtk::WrapMode::None
        });

        // Same line spacing as the task lists. The lines that come from wrapping a line get only
        // `pixels_inside_wrap`, so it's the space above and below a line, together.
        let above = metrics.text_offset() as i32;
        let below = (metrics.line_height - metrics.text_height) as i32 - above;
        self.text_view.set_pixels_above_lines(above);
        self.text_view.set_pixels_below_lines(below);
        self.text_view.set_pixels_inside_wrap(above + below);

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
