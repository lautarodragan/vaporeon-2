//! Tiny immediate-mode drawing layer on top of Cairo + Pango.
//!
//! Components lay themselves out in "rows" (one line of text each) and draw text with [`Canvas::text`].
//! Horizontal positions are in pixels, but paddings are usually expressed as a number of "columns",
//! (the width of an `M` in the configured font) through [`Canvas::cols`], so that the layout
//! resembles the one we had in the terminal.

use std::cell::RefCell;

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

pub struct Canvas<'a> {
    cr: &'a cairo::Context,
    layout: pango::Layout,
    font: pango::FontDescription,
    bold_font: pango::FontDescription,
    pub theme: &'a Theme,
    /// Height of a single row of text, in pixels.
    pub line_height: f64,
    /// Width of a single "column" (an `M`), in pixels.
    pub col_width: f64,
    pub width: f64,
    pub height: f64,
    clip_stack: RefCell<Vec<Rect>>,
}

/// Font metrics, used both for drawing and to know how many rows fit in the window.
#[derive(Debug, Copy, Clone)]
pub struct Metrics {
    pub line_height: f64,
    pub col_width: f64,
}

impl Metrics {
    pub fn new(pango_context: &pango::Context, theme: &Theme) -> Self {
        let layout = pango::Layout::new(pango_context);
        layout.set_font_description(Some(&theme.font_description()));
        layout.set_text("M");
        let (w, h) = layout.pixel_size();
        Self {
            line_height: (h as f64 * theme.line_height).round().max(1.0),
            col_width: (w as f64).max(1.0),
        }
    }
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
            line_height: metrics.line_height,
            col_width: metrics.col_width,
            width,
            height,
            clip_stack: RefCell::new(vec![]),
        }
    }

    /// Converts a number of "columns" to pixels.
    pub fn cols(&self, n: impl Into<f64>) -> f64 {
        n.into() * self.col_width
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

    /// Draws a single line of text with its top-left corner at (x, y), vertically centered in the row.
    /// Returns the width of the drawn text.
    pub fn text(&self, x: f64, y: f64, text: &str, style: Style) -> f64 {
        self.prepare_layout(text, &style);
        let (w, h) = self.layout.pixel_size();
        let color = if style.dim { style.fg.dimmed() } else { style.fg };
        self.set_color(color);
        self.cr.move_to(x, y + ((self.line_height - h as f64) / 2.0).floor());
        pangocairo::functions::show_layout(self.cr, &self.layout);
        w as f64
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

    /// X offset, in pixels, of the given byte index inside `text`.
    pub fn x_of_byte_index(&self, text: &str, style: &Style, byte_index: usize) -> f64 {
        self.prepare_layout(text, style);
        let index = byte_index.min(text.len()) as i32;
        let pos = self.layout.index_to_pos(index);
        pos.x() as f64 / pango::SCALE as f64
    }

    /// Draws the text caret (a thin vertical bar) at the given position.
    pub fn caret(&self, x: f64, y: f64, color: Color) {
        let h = self.line_height.min(self.layout_line_height() + 2.0);
        let y = y + ((self.line_height - h) / 2.0).floor();
        self.fill(
            Rect {
                x: x.round(),
                y,
                width: (self.col_width / 8.0).max(1.5).round(),
                height: h,
            },
            color,
        );
    }

    fn layout_line_height(&self) -> f64 {
        self.prepare_layout("M", &Style::new(Color::default()));
        self.layout.pixel_size().1 as f64
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
}
