//! Tiny immediate-mode drawing layer on top of Cairo + Pango.
//!
//! Components lay themselves out in rows (one line of text each, [`Canvas::line_height`] pixels tall)
//! and draw text with [`Canvas::text`]. Everything is measured in pixels.
//!
//! Text being edited is not drawn here: components call [`Canvas::set_editor`] instead, and a GTK text widget
//! is placed on top of the canvas at that position (see `main.rs`).

use std::cell::{Cell, RefCell};

use gtk::{cairo, pango};

use crate::theme::{Color, Theme};

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.width
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style {
    pub fg: Color,
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
}

impl Style {
    pub fn new(fg: Color) -> Self {
        Self {
            fg,
            bold: false,
            dim: false,
            underline: false,
        }
    }

    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    pub fn dim(self) -> Self {
        Self { dim: true, ..self }
    }

    pub fn dim_if(self, dim: bool) -> Self {
        Self { dim, ..self }
    }

    pub fn underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }
}

/// What kind of text is being edited.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EditTarget {
    /// The title of a task. Single line.
    TaskTitle,
    /// The description of a task, in the details panel. Multiple lines.
    TaskDetails,
}

/// Where the text editor widget should be.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EditorPlacement {
    pub target: EditTarget,
    pub rect: Rect,
}

/// Font metrics, used both for drawing and for placing the text editor widgets.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Metrics {
    /// Height of a row, in pixels.
    pub line_height: f64,
    /// Height of the text itself (Pango's logical height of a line), in pixels.
    pub text_height: f64,
}

impl Metrics {
    pub fn new(pango_context: &pango::Context, theme: &Theme) -> Self {
        let layout = pango::Layout::new(pango_context);
        layout.set_font_description(Some(&theme.font_description()));
        layout.set_text("M");
        let text_height = layout.pixel_size().1 as f64;
        Self {
            line_height: (text_height * theme.line_height).round().max(text_height),
            text_height,
        }
    }

    /// Distance from the top of a row to the top of its text. Text is vertically centered in its row.
    pub fn text_offset(&self) -> f64 {
        ((self.line_height - self.text_height) / 2.0).floor()
    }
}

pub struct Canvas<'a> {
    cr: &'a cairo::Context,
    layout: pango::Layout,
    font: pango::FontDescription,
    bold_font: pango::FontDescription,
    pub theme: &'a Theme,
    pub metrics: Metrics,
    /// Height of a single row of text, in pixels.
    pub line_height: f64,
    pub width: f64,
    pub height: f64,
    clip_stack: RefCell<Vec<Rect>>,
    editor: Cell<Option<EditorPlacement>>,
}

impl<'a> Canvas<'a> {
    pub fn new(
        cr: &'a cairo::Context,
        pango_context: &pango::Context,
        theme: &'a Theme,
        width: f64,
        height: f64,
    ) -> Self {
        let metrics = Metrics::new(pango_context, theme);
        let font = theme.font_description();
        let mut bold_font = font.clone();
        bold_font.set_weight(pango::Weight::Bold);
        let layout = pango::Layout::new(pango_context);

        Self {
            cr,
            layout,
            font,
            bold_font,
            theme,
            metrics,
            line_height: metrics.line_height,
            width,
            height,
            clip_stack: RefCell::new(vec![]),
            editor: Cell::new(None),
        }
    }

    /// Converts a number of rows to pixels.
    pub fn rows(&self, n: impl Into<f64>) -> f64 {
        n.into() * self.line_height
    }

    pub fn fill(&self, rect: Rect, color: Color) {
        self.set_color(color);
        self.cr.rectangle(rect.x, rect.y, rect.width, rect.height);
        let _ = self.cr.fill();
    }

    pub fn clear(&self, color: Color) {
        let _ = self.cr.save();
        self.cr.set_operator(cairo::Operator::Source);
        self.cr.set_source_rgba(color.r, color.g, color.b, color.a);
        let _ = self.cr.paint();
        let _ = self.cr.restore();
    }

    fn set_color(&self, color: Color) {
        self.cr.set_source_rgba(color.r, color.g, color.b, color.a);
    }

    fn prepare_layout(&self, text: &str, style: &Style) {
        self.layout
            .set_font_description(Some(if style.bold { &self.bold_font } else { &self.font }));
        self.layout.set_text(text);
        let attrs = pango::AttrList::new();
        if style.underline {
            attrs.insert(pango::AttrInt::new_underline(pango::Underline::Single));
        }
        self.layout.set_attributes(Some(&attrs));
    }

    /// Width of `text`, in pixels, if it were drawn with `style`.
    pub fn measure(&self, text: &str, style: &Style) -> f64 {
        self.prepare_layout(text, style);
        self.layout.pixel_size().0 as f64
    }

    /// Draws a single line of text in the row that starts at `y`. Returns the width of the drawn text.
    pub fn text(&self, x: f64, y: f64, text: &str, style: Style) -> f64 {
        self.prepare_layout(text, &style);
        let color = if style.dim { style.fg.dimmed() } else { style.fg };
        self.set_color(color);
        self.cr.move_to(x, y + self.metrics.text_offset());
        pangocairo::functions::show_layout(self.cr, &self.layout);
        self.layout.pixel_size().0 as f64
    }

    /// Draws several pieces of text next to each other. Returns the total width.
    pub fn spans(&self, x: f64, y: f64, spans: &[(&str, Style)]) -> f64 {
        spans.iter().fold(0.0, |width, (text, style)| {
            width + self.text(x + width, y, text, *style)
        })
    }

    pub fn measure_spans(&self, spans: &[(&str, Style)]) -> f64 {
        spans.iter().map(|(text, style)| self.measure(text, style)).sum()
    }

    /// Restricts drawing to `rect` until the matching [`Canvas::pop_clip`].
    pub fn push_clip(&self, rect: Rect) {
        let _ = self.cr.save();
        self.cr.rectangle(rect.x, rect.y, rect.width, rect.height);
        self.cr.clip();
        self.clip_stack.borrow_mut().push(rect);
    }

    pub fn pop_clip(&self) {
        if self.clip_stack.borrow_mut().pop().is_some() {
            let _ = self.cr.restore();
        }
    }

    /// Right edge of the current clip rectangle (or of the canvas, if there is none).
    pub fn clip_right(&self) -> f64 {
        self.clip_stack.borrow().last().map(Rect::right).unwrap_or(self.width)
    }

    /// Marks where the text editor widget should be placed. Text being edited is not drawn by the canvas.
    pub fn set_editor(&self, target: EditTarget, rect: Rect) {
        self.editor.set(Some(EditorPlacement { target, rect }));
    }

    pub fn editor(&self) -> Option<EditorPlacement> {
        self.editor.get()
    }
}
