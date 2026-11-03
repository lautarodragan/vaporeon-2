use super::editable_task::EditableTask;
use crate::{
    canvas::{Canvas, Rect, Style},
    components::FocusableWidget,
};

impl FocusableWidget for &EditableTask {
    fn render(self, canvas: &Canvas, area: &mut Rect, _skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let theme = canvas.theme;

        canvas.text(area.x, area.y, "-", Style::new(theme.list_decoration));

        let x = area.x + canvas.cols(2);
        let mut style = Style::new(theme.list_entry);

        if has_focus && !self.is_editing {
            style = style.underline();
        }

        let text = if has_focus && !self.is_editing && self.title.is_empty() {
            " "
        } else {
            self.title.as_str()
        };
        canvas.text(x, area.y, text, style);

        if has_focus && self.is_editing {
            let caret_x = x + canvas.x_of_byte_index(&self.title, &style, self.caret_byte_index());
            canvas.caret(caret_x, area.y, theme.caret);
        }
    }
}
