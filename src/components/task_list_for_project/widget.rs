use crate::{
    canvas::{Canvas, Rect, Style},
    components::{FocusableWidget, task_list_for_project::TaskListForProject},
};

impl FocusableWidget for &TaskListForProject {
    fn render(self, canvas: &Canvas, area: &mut Rect, skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        if self.show_title {
            if *skip == 0 {
                let style = Style::new(canvas.theme.list_header);
                canvas.text(area.x, area.y, self.project_name.as_str(), style);
                area.y += canvas.line_height;
            } else {
                *skip -= 1;
            }
        }

        if area.y >= canvas.height {
            return;
        }

        let left_padding = canvas.cols(self.left_padding());

        area.x += left_padding;
        for (i, task) in self.tasks.iter().enumerate() {
            if *skip > 0 {
                *skip -= 1;
                continue;
            }

            task.render(canvas, area, skip, has_focus && i == self.selected_task);
            area.y += canvas.line_height;

            if area.y >= canvas.height {
                break;
            }
        }
        area.x -= left_padding;
    }
}
