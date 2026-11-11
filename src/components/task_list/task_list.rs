use std::{collections::HashSet, path::Path};

use chrono::NaiveDate;
use uuid::Uuid;

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
                        id: Some(task.id()),
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

    /// Removes a task, and returns it. Returns None if there's no such task, or if it's empty.
    ///
    /// If it's the only task in its project, an empty one takes its place, since every project shows at least one
    /// (possibly empty) task.
    pub fn take_task(&mut self, status: usize, project: usize, task: usize) -> Option<EditableTask> {
        let project = self.status_lists.get_mut(status)?.tasks_by_project.get_mut(project)?;
        if project.tasks.get(task)?.is_empty() {
            return None;
        }

        if project.tasks.len() == 1 {
            return Some(std::mem::replace(&mut project.tasks[0], EditableTask::empty()));
        }

        let removed = project.tasks.remove(task);
        if project.selected_task > task || project.selected_task >= project.tasks.len() {
            project.selected_task = project.selected_task.saturating_sub(1);
        }
        Some(removed)
    }

    /// Whether any task (including the ones of hidden projects) has this id.
    pub fn contains_id(&self, id: Uuid) -> bool {
        self.status_lists
            .iter()
            .flat_map(|list| &list.tasks_by_project)
            .flat_map(|project| &project.tasks)
            .any(|task| task.id() == id)
            || self.hidden_tasks.iter().any(|task| task.id == Some(id))
    }

    /// Gives every task a new id. For tasks that are copies of others, e.g. from the template.
    pub fn new_ids(&mut self) {
        for list in &mut self.status_lists {
            for project in &mut list.tasks_by_project {
                project.tasks.iter_mut().for_each(EditableTask::new_id);
            }
        }
        for task in &mut self.hidden_tasks {
            task.id = Some(Uuid::new_v4());
        }
    }

    /// Inserts a task at `index` (clamped to the number of tasks) and selects it.
    /// If the project only has an empty placeholder task, that one is replaced instead.
    /// Returns false if there's no such status or project.
    pub fn insert_task(
        &mut self,
        status: usize,
        project: usize,
        index: usize,
        title: String,
        description: String,
    ) -> bool {
        self.insert(status, project, index, EditableTask::new(title, description))
    }

    /// Like [`TaskList::insert_task`], with a task that may come from somewhere else (keeping its id, unless a task
    /// here already has it).
    pub fn insert(&mut self, status: usize, project: usize, index: usize, mut task: EditableTask) -> bool {
        if self.contains_id(task.id()) {
            task.new_id();
        }
        let Some(project_list) = self
            .status_lists
            .get_mut(status)
            .and_then(|list| list.tasks_by_project.get_mut(project))
        else {
            return false;
        };

        let index = if let [only] = project_list.tasks.as_mut_slice()
            && only.is_empty()
            && only.description().trim().is_empty()
        {
            *only = task;
            0
        } else {
            let index = index.min(project_list.tasks.len());
            project_list.tasks.insert(index, task);
            index
        };

        self.select(status, project, index)
    }

    /// Indices of the selected status, project and task.
    pub fn selection(&self) -> (usize, usize, usize) {
        let status = self.selected_status();
        (
            self.selected_status_list,
            status.selected_project,
            status.selected_project().selected_task,
        )
    }

    /// Whether there is such a status and project.
    pub fn has_project(&self, status: usize, project: usize) -> bool {
        self.status_lists
            .get(status)
            .is_some_and(|list| project < list.tasks_by_project.len())
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
        self.load_day(Day::from_file_or_default(file_name));
    }

    /// Shows the tasks of `day`. The selection goes back to the first task.
    pub fn load_day(&mut self, mut day: Day) {
        // Every task needs an id, and a different one.
        let mut ids = HashSet::new();
        for task in &mut day.tasks {
            if !task.id.is_some_and(|id| ids.insert(id)) {
                let id = Uuid::new_v4();
                ids.insert(id);
                task.id = Some(id);
            }
        }

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
