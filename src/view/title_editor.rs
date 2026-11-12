//! Edits a task's title where it's shown, wrapping it like [`super::task_text::TaskText`] does: the task grows and
//! shrinks with the title while typing, and every line is exactly one row tall.
//!
//! A [`gtk::TextView`] on its own reports the height of its text as wrapped for the width it had before, so it lags
//! behind (and scrolls inside itself meanwhile). This measures the height itself, laying out the text with Pango for
//! the width it's given, like the text view will, and gives the text view exactly that.

use std::cell::Cell;

use gtk::{glib, pango, prelude::*, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TitleEditor {
        /// Height of every line, in pixels.
        pub line_height: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TitleEditor {
        const NAME: &'static str = "VaporeonTitleEditor";
        type Type = super::TitleEditor;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for TitleEditor {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for TitleEditor {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let widget = self.obj();
            let text_view = widget.text_view();
            // GTK wants children measured before they're allocated.
            let (child_min, child_natural, _, _) = text_view.measure(orientation, for_size);
            match orientation {
                // As narrow as needed: the text wraps.
                gtk::Orientation::Horizontal => (0, child_natural, -1, -1),
                _ => {
                    let lines = if for_size < 0 { 1 } else { widget.lines(for_size) };
                    let height = (lines as f64 * self.line_height.get()).ceil() as i32;
                    let height = height.max(child_min);
                    (height, height, -1, -1)
                }
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.obj().text_view().allocate(width, height, baseline, None);
        }
    }
}

glib::wrapper! {
    pub struct TitleEditor(ObjectSubclass<imp::TitleEditor>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TitleEditor {
    /// `text_height`: the height of the text itself, to center it in each line, like the title.
    pub fn new(line_height: f64, text_height: f64) -> Self {
        let widget: Self = glib::Object::new();
        widget.imp().line_height.set(line_height);
        widget.set_hexpand(true);

        // Same spacing as the title: the lines that come from wrapping get only `pixels_inside_wrap`.
        let above = ((line_height - text_height) / 2.0).floor() as i32;
        let below = (line_height - text_height) as i32 - above;
        let text_view = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .accepts_tab(false)
            .pixels_above_lines(above)
            .pixels_below_lines(below)
            .pixels_inside_wrap(above + below)
            .build();
        text_view.set_parent(&widget);

        // The height depends on the text.
        text_view.buffer().connect_changed(glib::clone!(
            #[weak]
            widget,
            move |_| widget.queue_resize()
        ));
        widget
    }

    pub fn text_view(&self) -> gtk::TextView {
        self.first_child()
            .and_downcast::<gtk::TextView>()
            .expect("the text view is the only child")
    }

    /// Lines the text takes at `width` (in pixels).
    fn lines(&self, width: i32) -> i32 {
        let text_view = self.text_view();
        let buffer = text_view.buffer();
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let layout = text_view.create_pango_layout(Some(&text));
        layout.set_width(width.max(1) * pango::SCALE);
        layout.set_wrap(pango::WrapMode::WordChar);
        layout.line_count().max(1)
    }
}
