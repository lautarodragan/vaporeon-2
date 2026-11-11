//! The text of a task in a list: its title, and after it, dimmed, a preview of its details.
//!
//! The title wraps (with `list_wrap` in the theme), every line exactly one row tall, so the lists keep their grid.
//! The preview goes right after the title, on its last line, and is cut short (`…`) where the line ends. That's why
//! this is a single widget that lays out the text with Pango, instead of two labels: a label next to another can't
//! start on the other's last line.

use std::cell::{Cell, RefCell};

use gtk::{gdk, glib, graphene, pango, prelude::*, subclass::prelude::*};

/// Below this many spaces left on the title's last line, the preview isn't shown: it would be little more than `…`.
const MIN_PREVIEW_SPACES: f64 = 3.0;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TaskText {
        pub title: RefCell<String>,
        pub preview: RefCell<String>,
        pub underline: Cell<bool>,
        pub wrap: Cell<bool>,
        /// Height of every line, in pixels.
        pub line_height: Cell<f64>,
        /// Width of a space, in pixels: between the title and the preview.
        pub space: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TaskText {
        const NAME: &'static str = "VaporeonTaskText";
        type Type = super::TaskText;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_css_name("tasktext");
            class.set_accessible_role(gtk::AccessibleRole::Label);
        }
    }

    impl ObjectImpl for TaskText {}

    impl WidgetImpl for TaskText {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let widget = self.obj();
            match orientation {
                // It can be as narrow as needed: the title wraps, or is cut short.
                gtk::Orientation::Horizontal => (0, widget.natural_width(), -1, -1),
                _ => {
                    let lines = if for_size < 0 { 1 } else { widget.text(for_size).lines };
                    let height = (lines as f64 * self.line_height.get()).ceil() as i32;
                    (height, height, -1, -1)
                }
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let text = widget.text(widget.width());
            let color = widget.color();
            snapshot.append_layout(&text.title, &color);
            if let Some((preview, x, y)) = text.preview {
                // Like `Color::dimmed` in the theme.
                let dim = gdk::RGBA::new(color.red(), color.green(), color.blue(), color.alpha() * 0.55);
                snapshot.save();
                snapshot.translate(&graphene::Point::new(x as f32, y as f32));
                snapshot.append_layout(&preview, &dim);
                snapshot.restore();
            }
        }
    }
}

glib::wrapper! {
    pub struct TaskText(ObjectSubclass<imp::TaskText>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// The text, laid out for a width.
struct Text {
    title: pango::Layout,
    /// Lines the title takes.
    lines: i32,
    /// The preview, and where it goes (in pixels).
    preview: Option<(pango::Layout, f64, f64)>,
}

impl TaskText {
    pub fn new() -> Self {
        let widget: Self = glib::Object::new();
        widget.imp().line_height.set(1.0);
        widget
    }

    /// What it shows. `underline` underlines the title (not the preview).
    pub fn set_content(&self, title: &str, preview: &str, underline: bool) {
        let imp = self.imp();
        let mut changed = false;
        if *imp.title.borrow() != title {
            imp.title.replace(title.to_string());
            self.update_property(&[gtk::accessible::Property::Label(title)]);
            changed = true;
        }
        if *imp.preview.borrow() != preview {
            imp.preview.replace(preview.to_string());
            changed = true;
        }
        if changed {
            self.queue_resize();
        } else if imp.underline.get() != underline {
            self.queue_draw();
        }
        imp.underline.set(underline);
    }

    /// How it's laid out: whether the title wraps, the height of a line, and the width of a space (in pixels).
    pub fn set_layout(&self, wrap: bool, line_height: f64, space: f64) {
        let imp = self.imp();
        let changed = imp.wrap.replace(wrap)
            != wrap | (imp.line_height.replace(line_height) != line_height) | (imp.space.replace(space) != space);
        if changed {
            self.queue_resize();
        }
    }

    fn layout(&self, text: &str, underline: bool) -> pango::Layout {
        let layout = self.create_pango_layout(Some(text));
        let attributes = pango::AttrList::new();
        let line_height = (self.imp().line_height.get() * pango::SCALE as f64).round() as i32;
        attributes.insert(pango::AttrInt::new_line_height_absolute(line_height));
        if underline {
            attributes.insert(pango::AttrInt::new_underline(pango::Underline::Single));
        }
        layout.set_attributes(Some(&attributes));
        layout
    }

    /// Laid out for `width` (in pixels).
    fn text(&self, width: i32) -> Text {
        let imp = self.imp();
        let title = self.layout(&imp.title.borrow(), imp.underline.get());
        title.set_width(width.max(1) * pango::SCALE);
        if imp.wrap.get() {
            title.set_wrap(pango::WrapMode::WordChar);
        } else {
            title.set_single_paragraph_mode(true);
            title.set_ellipsize(pango::EllipsizeMode::End);
        }
        let lines = title.line_count().max(1);

        let preview = (!imp.preview.borrow().is_empty())
            .then(|| {
                let space = imp.space.get();
                let last_line = title.line_readonly(lines - 1)?;
                let (_, extents) = last_line.extents();
                let x = (extents.x() + extents.width()) as f64 / pango::SCALE as f64 + space;
                let available = width as f64 - x;
                if available < MIN_PREVIEW_SPACES * space {
                    return None;
                }
                let preview = self.layout(&imp.preview.borrow(), false);
                preview.set_single_paragraph_mode(true);
                preview.set_ellipsize(pango::EllipsizeMode::End);
                preview.set_width((available * pango::SCALE as f64) as i32);
                Some((preview, x, (lines - 1) as f64 * imp.line_height.get()))
            })
            .flatten();

        Text { title, lines, preview }
    }

    /// Everything on one line.
    fn natural_width(&self) -> i32 {
        let imp = self.imp();
        let width = |text: &str| self.layout(text, false).pixel_size().0;
        let title = width(&imp.title.borrow());
        let preview = imp.preview.borrow();
        if preview.is_empty() {
            title
        } else {
            title + imp.space.get().ceil() as i32 + width(&preview)
        }
    }
}
