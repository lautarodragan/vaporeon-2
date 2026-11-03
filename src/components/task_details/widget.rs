use crate::{
    canvas::{Canvas, EditTarget, Rect, Style},
    components::{FocusableWidget, task_details::TaskDetails},
};

impl FocusableWidget for &TaskDetails {
    fn render(self, canvas: &Canvas, area: &mut Rect, _skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let style = Style::new(canvas.theme.list_header).dim_if(!has_focus);
        canvas.text(area.x, area.y, self.title.as_str(), style);

        area.y += canvas.rows(2);

        if has_focus && self.is_editing {
            // The text editor widget goes here, instead of the text.
            canvas.set_editor(EditTarget::TaskDetails, Rect {
                x: area.x,
                y: area.y,
                width: (canvas.clip_right() - area.x).max(0.0),
                height: (canvas.height - area.y).max(0.0),
            });
            return;
        }

        let style = Style::new(canvas.theme.list_entry).dim_if(!has_focus);
        for line in self.description.split('\n') {
            canvas.text(area.x, area.y, line, style);
            area.y += canvas.line_height;
        }
    }
}
