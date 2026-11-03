use crate::components::EditableTask;

pub struct TaskListForProject {
    pub project_id: usize,
    pub project_name: String,
    pub tasks: Vec<EditableTask>,
    pub selected_task: usize,
    pub show_title: bool,
}

impl TaskListForProject {
    pub fn selected_task(&self) -> &EditableTask {
        &self.tasks[self.selected_task]
    }

    pub fn selected_task_mut(&mut self) -> &mut EditableTask {
        &mut self.tasks[self.selected_task]
    }

    pub fn is_editing(&self) -> bool {
        self.tasks.iter().any(|task| task.is_editing())
    }

    pub fn height(&self) -> usize {
        1 + self.tasks.len()
    }

    pub fn left_padding(&self) -> u16 {
        if self.show_title { 1 } else { 0 }
    }

    pub fn cursor_y(&self) -> usize {
        self.selected_task + if self.show_title { 1 } else { 0 }
    }

    pub fn is_empty(&self) -> bool {
        !self.tasks.iter().any(|task| !task.is_empty())
    }

    pub fn select_last(&mut self) {
        self.selected_task = self.tasks.len().saturating_sub(1);
    }
}
