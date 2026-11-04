use std::cmp::Ordering;

use chrono::{Datelike, Days, Local, NaiveDate, Weekday};

use super::Root;
use crate::{
    canvas::{Canvas, Style},
    theme::Theme,
};

const WEEKDAYS: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// Minimum space between the items of the header, in pixels.
const HEADER_GAP: f64 = 40.0;

/// Rows used by the header (the "Project: ..." line plus an empty line), plus a little space above it.
pub const HEADER_ROWS: f64 = 2.35;

/// Something clickable in the header.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct HeaderLink {
    /// Horizontal range, in pixels.
    pub x: (f64, f64),
    pub date: NaiveDate,
}

impl Root {
    /// Renders the header: project filter on the left, selected date in the middle, and the week on the right.
    ///
    /// Returns the clickable parts: each day of the week goes to that day, and the arrows (shown when the selected
    /// day isn't in the current week) go to today.
    pub fn render_header(&self, canvas: &Canvas) -> Vec<HeaderLink> {
        let theme = canvas.theme;
        let left = 0.0;
        let right = canvas.width;
        let y = canvas.rows(0.35).round();

        // Project filter, on the left
        let header_style = Style::new(theme.list_entry);
        let project_name = self
            .project_filter
            .and_then(|i| self.settings.visible_projects().get(i).map(|p| p.name.clone()))
            .unwrap_or_else(|| "All".to_string());
        let project_width = canvas.spans(left, y, &[
            ("Project: ", header_style.dim()),
            (project_name.as_str(), header_style),
        ]);
        let free_left = left + project_width + HEADER_GAP;

        // Week quick view, on the right. Only if it fits.
        let day_names: Vec<String> = WEEKDAYS.iter().map(|d| d.to_string()).collect();
        let week = week_quick_view_spans(theme, self.selected_date, &day_names);
        let week_width = canvas.measure_spans(&week);
        let week_x = (right - week_width).round();
        let show_week = week_x >= free_left;
        let mut links = vec![];
        if show_week {
            canvas.spans(week_x, y, &week);
            links = week_links(canvas, week_x, self.selected_date, &week);
        }
        let free_right = if show_week { week_x - HEADER_GAP } else { right };

        // Selected date, centered. Only if it fits.
        // A dot after the date when it has unsaved changes.
        let mut date = self.selected_date.format("%Y-%m-%d").to_string();
        if self.is_dirty(super::DataFile::Day) {
            date.push_str(" •");
        }
        let date_style = Style::new(theme.list_header).bold();
        let date_width = canvas.measure(&date, &date_style);
        let date_x = ((canvas.width - date_width) / 2.0).round();
        if date_x >= free_left && date_x + date_width <= free_right {
            canvas.text(date_x, y, &date, date_style);
        }

        links
    }
}

/// Where each day of the week quick view was drawn (see [`week_quick_view_spans`] for the layout of the spans).
fn week_links(canvas: &Canvas, x: f64, selected_date: NaiveDate, spans: &[(&str, Style)]) -> Vec<HeaderLink> {
    let today = Local::now().date_naive();
    let monday = selected_date - Days::new(selected_date.weekday().num_days_from_monday() as u64);

    let mut links = vec![];
    let mut x = x;
    for (i, (text, style)) in spans.iter().enumerate() {
        let width = canvas.measure(text, style);
        let range = (x, x + width);
        x += width;

        let is_arrow = i == 0 || i == spans.len() - 1;
        if is_arrow {
            if !text.trim().is_empty() {
                links.push(HeaderLink { x: range, date: today });
            }
        } else if i % 2 == 1 {
            let day = (i - 1) / 2;
            links.push(HeaderLink {
                x: range,
                date: monday + Days::new(day as u64),
            });
        }
    }
    links
}

fn week_quick_view_spans<'a>(
    theme: &Theme,
    selected_date: NaiveDate,
    day_names: &'a [String],
) -> Vec<(&'a str, Style)> {
    let now = Local::now().date_naive();
    let selected_day = selected_date.weekday();
    let selected_date_week_ordering = selected_date.iso_week().cmp(&now.iso_week());

    let plain = Style::new(theme.list_entry);
    let mut spans: Vec<(&str, Style)> = vec![];

    spans.push(if selected_date_week_ordering == Ordering::Greater {
        ("« ", plain.dim())
    } else {
        ("  ", plain)
    });

    for (day, name) in WEEKDAYS.iter().zip(day_names.iter()) {
        let is_today = *day == now.weekday() && selected_date_week_ordering == Ordering::Equal;
        let color = if is_today { theme.list_header } else { theme.list_entry };
        let mut style = Style::new(color);
        if *day == selected_day {
            style = style.bold();
        } else if is_today {
            style = style.bold().dim();
        } else {
            style = style.dim();
        }
        spans.push((name.as_str(), style));
        spans.push((" ", plain));
    }

    spans.push(if selected_date_week_ordering == Ordering::Less {
        ("»", plain.dim())
    } else {
        (" ", plain)
    });

    spans
}

#[cfg(test)]
mod tests {
    use gtk::cairo;

    use super::*;
    use crate::settings::Settings;

    /// Renders the header of a Root with the given selected date, on a 1200px wide in-memory surface.
    fn header_links(selected_date: NaiveDate) -> Vec<HeaderLink> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1200, 60).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let pango_context = pangocairo::functions::create_context(&cr);
        let theme = Theme::default();
        let canvas = Canvas::new(&cr, &pango_context, &theme, 1200.0);
        let mut root = Root::new(Settings::default());
        root.selected_date = selected_date;
        root.render_header(&canvas)
    }

    #[test]
    fn week_days_link_to_their_dates() {
        let today = Local::now().date_naive();
        let links = header_links(today);
        // No arrows when looking at the current week: just the 7 days, Monday to Sunday, left to right.
        assert_eq!(links.len(), 7);
        let monday = today - Days::new(today.weekday().num_days_from_monday() as u64);
        for (i, link) in links.iter().enumerate() {
            assert_eq!(link.date, monday + Days::new(i as u64));
            assert!(link.x.0 < link.x.1);
            if i > 0 {
                assert!(links[i - 1].x.1 <= link.x.0);
            }
        }
    }

    #[test]
    fn arrows_link_to_today() {
        let today = Local::now().date_naive();
        for selected in [today - Days::new(30), today + Days::new(30)] {
            let links = header_links(selected);
            assert_eq!(links.len(), 8);
            assert_eq!(links.iter().filter(|link| link.date == today).count(), 1);
        }
    }
}
