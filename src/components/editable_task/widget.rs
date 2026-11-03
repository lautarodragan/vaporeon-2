use super::editable_task::EditableTask;
use crate::{
    canvas::{Canvas, EditTarget, Rect, Style},
    components::{FocusableWidget, spacing},
};

impl FocusableWidget for &EditableTask {
    fn render(self, canvas: &Canvas, area: &mut Rect, _skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let theme = canvas.theme;

        canvas.text(area.x, area.y, "-", Style::new(theme.list_decoration));

        let x = area.x + spacing::BULLET;

        if has_focus && self.is_editing {
            // The text editor widget goes here, instead of the text.
            canvas.set_editor(EditTarget::TaskTitle, Rect {
                x,
                y: area.y,
                width: (canvas.clip_right() - x).max(0.0),
                height: canvas.line_height,
            });
            return;
        }

        let mut style = Style::new(theme.list_entry);

        if has_focus {
            style = style.underline();
        }

        let text = if has_focus && self.title.is_empty() {
            " "
        } else {
            self.title.as_str()
        };
        canvas.text(x, area.y, text, style);
    }
}
