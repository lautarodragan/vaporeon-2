use std::cmp::Ordering;

use chrono::{Datelike, NaiveDate, Utc, Weekday};

use super::{RightPanel, Root};
use crate::{
    canvas::{Canvas, Rect, Style},
    components::FocusableWidget,
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

/// Below this width (in columns), only one panel is shown, using the full width.
const TWO_PANELS_MIN_WIDTH: f64 = 100.0;

/// Rows used by the header (the "Project: ..." line plus an empty line).
pub const HEADER_ROWS: f64 = 2.0;

impl Root {
    /// Horizontal padding, in pixels, on each side of the window.
    fn left_padding_px(&self, canvas: &Canvas) -> f64 {
        canvas.cols(self.left_padding)
    }

    /// How many task rows fit below the header.
    pub fn visible_rows(canvas: &Canvas) -> usize {
        ((canvas.height / canvas.line_height) - HEADER_ROWS).max(0.0) as usize
    }

    pub fn render(&self, canvas: &Canvas) {
        let theme = canvas.theme;
        let padding = self.left_padding_px(canvas);

        let mut area = Rect {
            x: padding,
            y: canvas.rows(0.35).round(),
            width: (canvas.width - padding * 2.0).max(0.0),
            height: canvas.height,
        };
        let width_in_cols = area.width / canvas.col_width;

        canvas.clear(theme.window_background());

        // Header: project filter
        let header_style = Style::new(theme.list_entry);
        let project_name = self
            .project_filter
            .and_then(|i| self.settings.visible_projects().get(i).map(|p| p.name.clone()))
            .unwrap_or_else(|| "All".to_string());
        canvas.spans(area.x, area.y, &[
            ("Project: ", header_style.dim()),
            (project_name.as_str(), header_style),
        ]);

        // Header: selected date, centered
        if width_in_cols >= 90.0 {
            let date = self.selected_date.format("%Y-%m-%d").to_string();
            let style = Style::new(theme.list_header).bold();
            let w = canvas.measure(&date, &style);
            canvas.text(((canvas.width - w) / 2.0).round(), area.y, &date, style);
        }

        // Header: week quick view, right-aligned
        if width_in_cols > 40.0 {
            render_week_quick_view(canvas, self.selected_date, area);
        }

        area.y += canvas.rows(HEADER_ROWS);
        area.height = (canvas.height - area.y).max(0.0);

        let single_panel = width_in_cols < TWO_PANELS_MIN_WIDTH;
        let show_left = !single_panel || self.selected_panel == 0;
        let show_right = !single_panel || self.selected_panel == 1;

        let (area_left, area_right) = if single_panel {
            (area, area)
        } else {
            let half = (area.width / 2.0).round();
            (Rect { width: half, ..area }, Rect {
                x: area.x + half,
                width: area.width - half,
                ..area
            })
        };

        if show_left {
            let mut area = area_left;
            canvas.push_clip(area);
            self.task_list_day
                .render(canvas, &mut area, &mut 0, self.selected_panel == 0);
            canvas.pop_clip();
        }

        if show_right {
            let mut area = area_right;
            canvas.push_clip(area);
            if self.right_panel == RightPanel::GlobalTaskList {
                self.task_list_global
                    .render(canvas, &mut area, &mut 0, self.selected_panel == 1);
            } else if self.right_panel == RightPanel::TaskDetails {
                self.task_details
                    .render(canvas, &mut area, &mut 0, self.selected_panel == 1);
            }
            canvas.pop_clip();
        }
    }
}

fn render_week_quick_view(canvas: &Canvas, selected_date: NaiveDate, area: Rect) {
    let theme = canvas.theme;
    let now = Utc::now().date_naive();
    let selected_day = selected_date.weekday();
    let selected_date_week_ordering = selected_date.iso_week().cmp(&now.iso_week());

    let day_names: Vec<String> = WEEKDAYS.iter().map(|d| d.to_string()).collect();
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

    let width = canvas.measure_spans(&spans);
    canvas.spans((area.right() - width).round(), area.y, &spans);
}
