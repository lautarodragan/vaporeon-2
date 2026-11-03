use super::TaskList;
use crate::{
    canvas::{Canvas, Rect},
    components::FocusableWidget,
};

impl FocusableWidget for &TaskList {
    fn render(self, canvas: &Canvas, area: &mut Rect, _skip: &mut usize, has_focus: bool)
    where
        Self: Sized,
    {
        let mut skip = self.scroll;

        for (i, list) in self.status_lists.iter().enumerate() {
            list.render(canvas, area, &mut skip, has_focus && i == self.selected_status_list);
        }
    }
}
