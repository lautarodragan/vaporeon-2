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

    pub fn editing_task_mut(&mut self) -> Option<&mut EditableTask> {
        self.tasks.iter_mut().find(|task| task.is_editing())
    }

    pub fn is_empty(&self) -> bool {
        !self.tasks.iter().any(|task| !task.is_empty())
    }

    pub fn select_last(&mut self) {
        self.selected_task = self.tasks.len().saturating_sub(1);
    }
}
