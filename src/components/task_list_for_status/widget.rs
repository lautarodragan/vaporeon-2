use super::TaskListForStatus;
use crate::{
    canvas::{Canvas, Rect, Style},
    components::{FocusableWidget, spacing},
};

impl FocusableWidget for &TaskListForStatus {
    fn render(self, canvas: &Canvas, area: &mut Rect, skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        if *skip == 0 {
            let style = Style::new(canvas.theme.list_header);
            canvas.text(area.x, area.y, self.status.as_str(), style);
            area.y += canvas.line_height;
        } else {
            *skip -= 1;
        }

        let render_left_border = self.project_filter.is_none();
        let left_padding = self.left_padding();
        let border_x = area.x;

        area.x += left_padding;

        for (i, project) in self.tasks_by_project.iter().enumerate() {
            if let Some(project_index) = self.project_filter
                && i != project_index
            {
                continue;
            }

            let start = area.y;

            project.render(canvas, area, skip, has_focus && i == self.selected_project);

            if *skip > 0 {
                *skip -= 1;
            } else {
                area.y += canvas.line_height;
            }

            if render_left_border {
                let mut end = area.y.min(canvas.height);
                if i == self.tasks_by_project.len() - 1 {
                    end -= canvas.line_height;
                }
                if end > start {
                    let width = spacing::LINE_WIDTH;
                    canvas.fill(
                        Rect {
                            x: border_x + spacing::STATUS - width,
                            y: start,
                            width,
                            height: end - start,
                        },
                        canvas.theme.list_left_line.dimmed(),
                    );
                }
            }
        }

        area.x -= left_padding;
    }
}
