use std::cmp::Ordering;

use chrono::{Datelike, Local, NaiveDate, Weekday};

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

impl Root {
    /// Renders the header: project filter on the left, selected date in the middle, and the week on the right.
    pub fn render_header(&self, canvas: &Canvas) {
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
        if show_week {
            canvas.spans(week_x, y, &week);
        }
        let free_right = if show_week { week_x - HEADER_GAP } else { right };

        // Selected date, centered. Only if it fits.
        let date = self.selected_date.format("%Y-%m-%d").to_string();
        let date_style = Style::new(theme.list_header).bold();
        let date_width = canvas.measure(&date, &date_style);
        let date_x = ((canvas.width - date_width) / 2.0).round();
        if date_x >= free_left && date_x + date_width <= free_right {
            canvas.text(date_x, y, &date, date_style);
        }
    }
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
