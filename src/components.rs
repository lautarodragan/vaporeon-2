use crate::canvas::{Canvas, Rect};

mod editable_task;
pub mod root;
pub mod task_details;
pub mod task_list;
pub mod task_list_for_project;
pub mod task_list_for_status;
pub mod textarea;

pub use editable_task::EditableTask;

pub trait FocusableWidget {
    /// Renders the widget starting at the top-left of `area`, advancing `area.y` one row for each line rendered.
    ///
    /// `skip` is the number of rows that are scrolled out of view: rows are skipped (and `skip` decremented)
    /// instead of rendered until it reaches zero.
    fn render(self, canvas: &Canvas, area: &mut Rect, skip: &mut usize, has_focus: bool)
    where
        Self: Sized;
}
