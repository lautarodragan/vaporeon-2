//! GTK text widgets used to edit text, placed on top of the canvas.
//!
//! The canvas doesn't draw text that is being edited. Instead, while rendering, components report where that text
//! would be (see [`Canvas::set_editor`](crate::canvas::Canvas::set_editor)), and after the frame is drawn the
//! corresponding widget is moved there, styled to look just like the canvas' text:
//!
//! - task titles: a [`gtk::Text`] (single line),
//! - task details: a [`gtk::TextView`] (multiple lines).
//!
//! The widgets own the text while editing (caret, selection, clipboard, input methods, undo, etc.) and report every
//! change back to the model with [`Root::set_editing_text`](crate::components::root::Root::set_editing_text).

use std::cell::Cell;

use gtk::{gdk, glib::translate::IntoGlib, pango, prelude::*};

use crate::{
    AppRef,
    canvas::{EditTarget, EditorPlacement, Metrics},
    theme::{Color, Theme},
};

pub struct Editors {
    pub drawing_area: gtk::DrawingArea,
    /// Layer on top of the drawing area, where the editors are placed.
    pub fixed: gtk::Fixed,
    title: gtk::Text,
    details: gtk::TextView,
    details_scroll: gtk::ScrolledWindow,
    css: gtk::CssProvider,
    /// True while we set the editors' text ourselves, so their "changed" handlers can ignore it.
    syncing: Cell<bool>,
    /// Where the canvas wants the editor to be, according to the last frame drawn.
    desired: Cell<Option<EditorPlacement>>,
    /// Where the editor actually is.
    applied: Cell<Option<EditorPlacement>>,
    sync_scheduled: Cell<bool>,
}

impl Editors {
    pub fn new(drawing_area: &gtk::DrawingArea) -> Self {
        let fixed = gtk::Fixed::new();

        let title = gtk::Text::builder().visible(false).build();
        title.add_css_class("vaporeon-editor");

        let details = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::None)
            .accepts_tab(false)
            .build();
        details.add_css_class("vaporeon-editor");

        let details_scroll = gtk::ScrolledWindow::builder().child(&details).visible(false).build();
        details_scroll.add_css_class("vaporeon-editor");

        fixed.put(&title, 0.0, 0.0);
        fixed.put(&details_scroll, 0.0, 0.0);

        let css = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        Self {
            drawing_area: drawing_area.clone(),
            fixed,
            title,
            details,
            details_scroll,
            css,
            syncing: Cell::new(false),
            desired: Cell::new(None),
            applied: Cell::new(None),
            sync_scheduled: Cell::new(false),
        }
    }

    /// Sends every change made in the editors back to the model.
    pub fn connect(self: &std::rc::Rc<Self>, app: &AppRef) {
        self.title.connect_changed({
            let app = app.clone();
            let editors = std::rc::Rc::downgrade(self);
            move |title| {
                let Some(editors) = editors.upgrade() else { return };
                if editors.syncing.get() {
                    return;
                }
                app.borrow_mut().root.set_editing_text(title.text().to_string());
                editors.drawing_area.queue_draw();
            }
        });

        self.details.buffer().connect_changed({
            let app = app.clone();
            let editors = std::rc::Rc::downgrade(self);
            move |buffer| {
                let Some(editors) = editors.upgrade() else { return };
                if editors.syncing.get() {
                    return;
                }
                let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                app.borrow_mut().root.set_editing_text(text.to_string());
                editors.drawing_area.queue_draw();
            }
        });
    }

    /// Makes the editors look like the canvas' text.
    pub fn apply_theme(&self, theme: &Theme) {
        self.css.load_from_string(&editor_css(theme));
    }

    /// Called after every frame, with the editor placement reported by the canvas.
    /// If it changed, the editors are updated as soon as GTK is idle (widgets can't be moved while drawing).
    pub fn after_draw(self: &std::rc::Rc<Self>, app: &AppRef, placement: Option<EditorPlacement>) {
        self.desired.set(placement);
        if placement != self.applied.get() && !self.sync_scheduled.replace(true) {
            let app = app.clone();
            let editors = self.clone();
            gtk::glib::idle_add_local_once(move || editors.sync(&app));
        }
    }

    /// Hides the editors right away. Used when editing is confirmed or cancelled.
    pub fn hide(&self) {
        self.applied.set(None);
        self.title.set_visible(false);
        self.details_scroll.set_visible(false);
        self.drawing_area.grab_focus();
    }

    fn sync(&self, app: &AppRef) {
        self.sync_scheduled.set(false);

        let desired = self.desired.get();
        let applied = self.applied.get();
        if desired == applied {
            return;
        }

        let Some(placement) = desired else {
            self.hide();
            return;
        };

        self.applied.set(desired);

        let is_new = applied.map(|p| p.target) != Some(placement.target);
        let metrics = Metrics::new(&self.drawing_area.pango_context(), &app.borrow().theme);
        let rect = placement.rect;

        // Note: setting the editors' text emits "changed", whose handler borrows `app`,
        // so we must not be holding a borrow while doing it.
        match placement.target {
            EditTarget::TaskTitle => {
                self.details_scroll.set_visible(false);
                if is_new {
                    let text = app.borrow_mut().root.editing_text();
                    self.syncing.set(true);
                    self.title.set_text(&text);
                    self.syncing.set(false);
                    self.title.set_visible(true);
                    self.title.grab_focus_without_selecting();
                    self.title.set_position(-1);
                }
                self.title
                    .set_size_request(rect.width.max(1.0) as i32, metrics.text_height as i32);
                self.fixed.move_(&self.title, rect.x, rect.y + metrics.text_offset());
            }
            EditTarget::TaskDetails => {
                self.title.set_visible(false);
                if is_new {
                    let text = app.borrow_mut().root.editing_text();
                    let buffer = self.details.buffer();
                    self.syncing.set(true);
                    buffer.set_text(&text);
                    self.syncing.set(false);
                    buffer.place_cursor(&buffer.end_iter());
                    self.details_scroll.set_visible(true);
                    self.details.grab_focus();
                }
                // Same line spacing as the canvas.
                let offset = metrics.text_offset();
                self.details.set_pixels_above_lines(offset as i32);
                self.details
                    .set_pixels_below_lines((metrics.line_height - metrics.text_height - offset) as i32);
                self.details_scroll
                    .set_size_request(rect.width.max(1.0) as i32, rect.height.max(1.0) as i32);
                self.fixed.move_(&self.details_scroll, rect.x, rect.y);
            }
        }
    }
}

fn css_color(color: Color) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        (color.r * 255.0).round(),
        (color.g * 255.0).round(),
        (color.b * 255.0).round(),
        color.a
    )
}

fn editor_css(theme: &Theme) -> String {
    let font = theme.font_description();
    let family = font
        .family()
        .map(|f| f.replace('"', ""))
        .unwrap_or_else(|| "monospace".to_string());
    let size = font.size() as f64 / pango::SCALE as f64;
    let size = if size <= 0.0 {
        String::new()
    } else if font.is_size_absolute() {
        format!("font-size: {size}px;")
    } else {
        format!("font-size: {size}pt;")
    };
    let weight = font.weight().into_glib();
    let style = match font.style() {
        pango::Style::Italic => "italic",
        pango::Style::Oblique => "oblique",
        _ => "normal",
    };

    format!(
        "
.vaporeon-editor,
.vaporeon-editor > text,
.vaporeon-editor > viewport {{
    background: none;
    background-color: transparent;
    border: none;
    box-shadow: none;
    outline: none;
    padding: 0;
    margin: 0;
    min-height: 0;
    min-width: 0;
    color: {color};
    caret-color: {caret};
    font-family: \"{family}\";
    {size}
    font-weight: {weight};
    font-style: {style};
}}

.vaporeon-editor selection,
.vaporeon-editor > text selection {{
    background-color: {selection};
    color: {color};
}}
",
        color = css_color(theme.list_entry),
        caret = css_color(theme.caret),
        selection = css_color(theme.list_header.with_alpha(0.35)),
    )
}
