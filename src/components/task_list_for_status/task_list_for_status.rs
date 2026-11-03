use crate::{
    components::{EditableTask, task_list_for_project::TaskListForProject},
    models::{Project, Task, TaskStatus},
};

pub struct TaskListForStatus {
    pub status: TaskStatus,
    pub tasks_by_project: Vec<TaskListForProject>,
    pub projects: Vec<Project>,
    pub selected_project: usize,
    pub project_filter: Option<usize>,
    left_padding: u16,
}

impl TaskListForStatus {
    pub fn is_editing(&self) -> bool {
        self.tasks_by_project.iter().any(|project| project.is_editing())
    }

    pub fn from_tasks(projects: Vec<Project>, tasks: Vec<Task>) -> Vec<Self> {
        TaskStatus::all()
            .into_iter()
            .map(|task_status| TaskListForStatus {
                tasks_by_project: projects
                    .iter()
                    .map(|project| {
                        let mut tasks: Vec<_> = tasks
                            .iter()
                            .filter(|task| task.status == task_status && task.project_id == project.id)
                            .map(|task| EditableTask::from_day_task(task))
                            .collect();

                        if tasks.is_empty() {
                            tasks.push(EditableTask::empty());
                        }

                        TaskListForProject {
                            project_id: project.id,
                            project_name: project.name.clone(),
                            tasks,
                            selected_task: 0,
                            show_title: true,
                        }
                    })
                    .collect(),
                status: task_status,
                projects: projects.clone(),
                selected_project: 0,
                project_filter: None,
                left_padding: 1,
            })
            .collect()
    }

    pub fn left_padding(&self) -> u16 {
        let render_left_border = self.project_filter.is_none();
        if render_left_border {
            self.left_padding + 2
        } else {
            self.left_padding
        }
    }

    pub fn height(&self) -> usize {
        1 + if self.project_filter.is_none() {
            self.tasks_by_project.iter().map(|p| p.height() + 1).sum::<usize>()
        } else {
            self.tasks_by_project[self.selected_project].height()
        }
    }

    pub fn cursor_y(&self) -> usize {
        1 + if self.project_filter.is_none() {
            self.tasks_by_project
                .iter()
                .take(self.selected_project)
                .map(|p| p.height() + 1)
                .sum::<usize>()
                + self.tasks_by_project[self.selected_project].cursor_y()
        } else {
            self.tasks_by_project[self.selected_project].cursor_y()
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.tasks_by_project.iter().any(|tbp| !tbp.is_empty())
    }

    pub fn set_filter(&mut self, project_index: Option<usize>) {
        self.project_filter = project_index;
        if let Some(i) = project_index {
            self.selected_project = i;
        }
        for list in &mut self.tasks_by_project {
            list.show_title = project_index.is_none();
        }
    }

    pub fn selected_project(&self) -> &TaskListForProject {
        &self.tasks_by_project[self.selected_project]
    }

    pub fn selected_project_mut(&mut self) -> &mut TaskListForProject {
        &mut self.tasks_by_project[self.selected_project]
    }

    pub fn selected_task(&self) -> &EditableTask {
        self.selected_project().selected_task()
    }

    pub fn select_first(&mut self) {
        if let Some(i) = self.project_filter {
            self.selected_project = i;
        } else {
            self.selected_project = 0;
        }
    }

    pub fn select_last(&mut self) {
        if let Some(i) = self.project_filter {
            self.selected_project = i;
        } else {
            self.selected_project = self.tasks_by_project.len().saturating_sub(1);
        }
    }
}
