use crate::canvas::{Canvas, Rect};

mod editable_task;
pub mod root;
pub mod task_details;
pub mod task_list;
pub mod task_list_for_project;
pub mod task_list_for_status;

pub use editable_task::EditableTask;

/// Horizontal spacing used by the layout, in pixels.
pub mod spacing {
    /// Left and right padding of the whole window.
    pub const WINDOW: f64 = 20.0;
    /// Indentation of the projects inside a status list ("TODO", "DONE", etc.).
    pub const STATUS: f64 = 10.0;
    /// Extra indentation of the projects when the vertical line on their left is drawn.
    pub const STATUS_LINE: f64 = 20.0;
    /// Indentation of the tasks inside a project.
    pub const PROJECT: f64 = 10.0;
    /// Space taken by the `-` before each task.
    pub const BULLET: f64 = 20.0;
    /// Width of the vertical line on the left of the projects.
    pub const LINE_WIDTH: f64 = 1.0;
    /// Minimum space between the items of the header.
    pub const HEADER_GAP: f64 = 40.0;
}

pub trait FocusableWidget {
    /// Renders the widget starting at the top-left of `area`, advancing `area.y` one row for each line rendered.
    ///
    /// `skip` is the number of rows that are scrolled out of view: rows are skipped (and `skip` decremented)
    /// instead of rendered until it reaches zero.
    fn render(self, canvas: &Canvas, area: &mut Rect, skip: &mut usize, has_focus: bool)
    where
        Self: Sized;
}
