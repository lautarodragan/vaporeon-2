use std::path::Path;

use chrono::NaiveDate;

use crate::{
    components::{EditableTask, task_list_for_status::TaskListForStatus},
    models::{Day, Task},
    settings::Settings,
};

pub struct TaskList {
    pub(super) settings: Settings,
    pub(super) status_lists: Vec<TaskListForStatus>,
    pub(super) selected_status_list: usize,
    pub(super) project_filter: Option<usize>,
    /// Tasks belonging to hidden projects. They are not shown in the UI, but are
    /// kept around so they survive a save instead of being silently dropped.
    hidden_tasks: Vec<Task>,
}

impl TaskList {
    pub fn new(settings: Settings) -> Self {
        Self {
            status_lists: TaskListForStatus::from_tasks(settings.visible_projects(), vec![]),
            selected_status_list: 0,
            settings,
            project_filter: None,
            hidden_tasks: vec![],
        }
    }

    pub fn status_lists_mut(&mut self) -> &mut Vec<TaskListForStatus> {
        &mut self.status_lists
    }

    pub fn status_lists(&self) -> &[TaskListForStatus] {
        &self.status_lists
    }

    /// Index of the selected status list.
    pub fn selected_status_index(&self) -> usize {
        self.selected_status_list
    }

    pub fn project_filter(&self) -> Option<usize> {
        self.project_filter
    }

    /// Selects a task, given its indices. Returns false (and does nothing) if they're out of range.
    pub fn select(&mut self, status: usize, project: usize, task: usize) -> bool {
        let Some(list) = self.status_lists.get_mut(status) else {
            return false;
        };
        let Some(project_list) = list.tasks_by_project.get_mut(project) else {
            return false;
        };
        if task >= project_list.tasks.len() {
            return false;
        }
        project_list.selected_task = task;
        list.selected_project = project;
        self.selected_status_list = status;
        true
    }

    pub fn is_editing(&self) -> bool {
        self.status_lists.iter().any(|list| list.is_editing())
    }

    pub fn to_day(&self) -> Day {
        let mut tasks: Vec<Task> = self
            .status_lists
            .iter()
            .flat_map(|list| {
                list.tasks_by_project.iter().flat_map(|project| {
                    project.tasks.iter().map(|task| Task {
                        text: task.title().to_string(),
                        project_id: project.project_id,
                        status: list.status.clone(),
                        details: Some(task.description().to_string()),
                    })
                })
            })
            .collect();
        // Re-attach tasks of hidden projects so they are preserved across a save.
        tasks.extend(self.hidden_tasks.iter().cloned());
        Day {
            date_time: NaiveDate::default(), // TODO: remove from Day thingy
            tasks,
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.status_lists.iter().any(|tbs| !tbs.is_empty())
    }

    /// Row (counting from the top of the list) of the selected task.
    pub fn cursor_y(&self) -> usize {
        self.status_lists
            .iter()
            .take(self.selected_status_list)
            .map(|l| l.height())
            .sum::<usize>()
            + self.status_lists[self.selected_status_list].cursor_y()
    }

    pub fn editing_task_mut(&mut self) -> Option<&mut EditableTask> {
        self.status_lists.iter_mut().find_map(|list| list.editing_task_mut())
    }

    pub fn set_filter(&mut self, project_index: Option<usize>) {
        self.project_filter = project_index;
        for list in &mut self.status_lists {
            list.set_filter(project_index);
        }
    }

    pub fn selected_status(&self) -> &TaskListForStatus {
        &self.status_lists[self.selected_status_list]
    }

    pub fn selected_status_mut(&mut self) -> &mut TaskListForStatus {
        &mut self.status_lists[self.selected_status_list]
    }

    pub fn load_file(&mut self, file_name: &Path) {
        let day = Day::from_file_or_default(file_name);

        let visible_projects = self.settings.visible_projects();
        // Tasks whose project is hidden (or no longer exists as a visible project)
        // are set aside so they don't show up in the UI but aren't lost on save.
        let (tasks, hidden_tasks): (Vec<Task>, Vec<Task>) = day
            .tasks
            .into_iter()
            .partition(|task| visible_projects.iter().any(|p| p.id == task.project_id));
        self.hidden_tasks = hidden_tasks;

        self.status_lists = TaskListForStatus::from_tasks(visible_projects, tasks);
        for list in &mut self.status_lists {
            list.set_filter(self.project_filter);
        }
    }
}
