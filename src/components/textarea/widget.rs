use crate::{
    canvas::{Canvas, Rect, Style},
    components::{FocusableWidget, textarea::Textarea},
};

impl FocusableWidget for &Textarea {
    fn render(self, canvas: &Canvas, area: &mut Rect, _skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let style = Style::new(canvas.theme.list_entry).dim_if(!has_focus);

        for (i, line) in self.lines.iter().enumerate() {
            canvas.text(area.x, area.y, line.as_str(), style);

            if has_focus && self.is_editing && i == self.caret_position.line {
                let caret_x = area.x + canvas.x_of_byte_index(line, &style, self.caret_byte_index());
                canvas.caret(caret_x, area.y, canvas.theme.caret);
            }

            area.y += canvas.line_height;
        }
    }
}
