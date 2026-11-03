use crate::{
    canvas::{Canvas, Rect, Style},
    components::{FocusableWidget, task_details::TaskDetails},
};

impl FocusableWidget for &TaskDetails {
    fn render(self, canvas: &Canvas, area: &mut Rect, skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let style = Style::new(canvas.theme.list_header).dim_if(!has_focus);
        canvas.text(area.x, area.y, self.title.as_str(), style);

        area.y += canvas.rows(2);

        self.description.render(canvas, area, skip, has_focus);
    }
}
