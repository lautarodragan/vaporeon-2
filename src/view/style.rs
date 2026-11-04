//! The theme, as GTK CSS.

use gtk::{glib::translate::IntoGlib, pango};

use crate::{
    canvas::Metrics,
    theme::{Color, Theme},
};

/// Left and right padding of the whole window, in pixels.
pub const WINDOW_PADDING: i32 = 20;
/// Indentation of the projects inside a status list ("TODO", "DONE", etc.).
const STATUS_INDENT: i32 = 10;
/// Extra indentation of the projects when the vertical line on their left is shown.
const STATUS_LINE_INDENT: i32 = 20;
/// Width of the vertical line on the left of the projects.
const LINE_WIDTH: i32 = 1;
/// Indentation of the tasks inside a project.
const PROJECT_INDENT: i32 = 10;
/// Space taken by the `-` before each task.
pub const BULLET_WIDTH: i32 = 20;

fn css_color(color: Color) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        (color.r * 255.0).round(),
        (color.g * 255.0).round(),
        (color.b * 255.0).round(),
        color.a
    )
}

fn css_font(font: &pango::FontDescription) -> String {
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
    format!("font-family: \"{family}\"; {size} font-weight: {weight}; font-style: {style};")
}

pub fn css(theme: &Theme, metrics: &Metrics) -> String {
    let background = css_color(theme.window_background());
    let entry = css_color(theme.list_entry);
    let entry_dim = css_color(theme.list_entry.dimmed());
    let header = css_color(theme.list_header);
    let header_dim = css_color(theme.list_header.dimmed());
    let decoration = css_color(theme.list_decoration);
    let line = css_color(theme.list_left_line.dimmed());
    let caret = css_color(theme.caret);
    let selection = css_color(theme.list_header.with_alpha(0.35));
    let font = css_font(&theme.font_description());
    let row_height = metrics.line_height;
    let line_margin = STATUS_INDENT - LINE_WIDTH;

    format!(
        "
/* The window is transparent; this is the only background. */
window.vaporeon {{
    background: none;
    background-color: {background};
    box-shadow: none;
}}

window.vaporeon box,
window.vaporeon stack,
window.vaporeon scrolledwindow,
window.vaporeon viewport,
window.vaporeon textview,
window.vaporeon textview > text,
window.vaporeon text {{
    background: none;
    background-color: transparent;
    border: none;
    box-shadow: none;
    outline: none;
}}

/* Scrollbars: a thin line in the same color as the line on the left of the projects. */
window.vaporeon scrollbar {{
    background: none;
    border: none;
}}

window.vaporeon scrollbar slider {{
    background-color: {line};
    border: none;
    min-width: 2px;
    min-height: 2px;
}}

window.vaporeon scrolledwindow undershoot,
window.vaporeon scrolledwindow overshoot {{
    background: none;
    box-shadow: none;
}}

window.vaporeon label,
window.vaporeon text,
window.vaporeon textview {{
    {font}
    color: {entry};
    caret-color: {caret};
    padding: 0;
    margin: 0;
    min-height: 0;
    min-width: 0;
}}

window.vaporeon selection {{
    background-color: {selection};
    color: {entry};
}}

/* Every line of the lists is exactly one row tall. */
window.vaporeon .row {{
    min-height: {row_height}px;
}}

window.vaporeon .header {{
    color: {header};
}}

window.vaporeon .header.dim {{
    color: {header_dim};
}}

window.vaporeon textview.dim > text {{
    color: {entry_dim};
}}

window.vaporeon .bullet {{
    color: {decoration};
    min-width: {BULLET_WIDTH}px;
}}

window.vaporeon .project {{
    margin-left: {STATUS_INDENT}px;
}}

window.vaporeon .project.line {{
    margin-left: {line_margin}px;
    border-left: {LINE_WIDTH}px solid {line};
    padding-left: {STATUS_LINE_INDENT}px;
}}

window.vaporeon .tasks.indented {{
    margin-left: {PROJECT_INDENT}px;
}}

/* Status bar: messages and questions, at the bottom. */
window.vaporeon .status-bar .options {{
    margin-left: 20px;
}}

window.vaporeon .status-bar .option {{
    margin-right: 20px;
}}

window.vaporeon .status-bar .key {{
    color: {header};
    margin-right: 6px;
}}

/* Where a dragged task would be dropped. A 2px line centered on the border between rows (1px inside the row,
   1px outside), so below a row and above the next one are drawn in exactly the same place. */
window.vaporeon .task.drop-above {{
    box-shadow: inset 0 1px {header}, 0 -1px {header};
}}

window.vaporeon .task.drop-below {{
    box-shadow: inset 0 -1px {header}, 0 1px {header};
}}

window.vaporeon .task.selected > label.title {{
    text-decoration-line: underline;
}}
"
    )
}
