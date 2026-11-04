use std::{fs::remove_file, io::ErrorKind};

use chrono::{Days, Local, NaiveDate};

use crate::{
    action::{Action, OnActions, TextAction},
    components::{
        EditableTask,
        task_details::TaskDetails,
        task_list::TaskList,
        task_list_for_status::TaskListForStatus,
    },
    git,
    models::{Day, TaskStatus},
    settings::Settings,
};

pub struct Root {
    pub(super) settings: Settings,
    pub(super) task_list_day: TaskList,
    pub(super) task_list_global: TaskList,
    pub(super) task_details: TaskDetails,
    pub(super) right_panel: RightPanel,
    pub(super) selected_panel: usize, // 0 = selected day, 1 = global
    pub(super) selected_date: NaiveDate,
    pub(super) selected_status_list: usize,
    pub(super) project_filter: Option<usize>,
}

impl Root {
    pub fn new(settings: Settings) -> Self {
        Self {
            task_list_day: TaskList::new(settings.clone()),
            task_list_global: TaskList::new(settings.clone()),
            task_details: TaskDetails::new(),
            settings,
            selected_panel: 0,
            selected_date: Local::now().date_naive(),
            selected_status_list: 0,
            project_filter: None,
            right_panel: RightPanel::Nothing,
        }
    }

    fn file_name(&self) -> String {
        self.selected_date.format("%Y-%m-%d").to_string()
    }

    pub fn to_day(&self) -> Day {
        let day = self.task_list_day.to_day();
        Day {
            date_time: self.selected_date,
            ..day
        }
    }

    pub fn to_day_global(&self) -> Day {
        self.task_list_global.to_day()
    }

    pub fn load_selected_day(&mut self) {
        let file_name = self.file_name();
        let path = self.settings.storage_path.join(file_name + ".toml");
        self.task_list_day.load_file(path.as_path());
    }

    pub fn load_from_template(&mut self) {
        let file_name = "template".to_string();
        let path = self.settings.storage_path.join(file_name + ".toml");
        self.task_list_day.load_file(path.as_path());
    }

    pub fn load_global(&mut self) {
        let file_name = "global".to_string();
        let path = self.settings.storage_path.join(file_name + ".toml");
        self.task_list_global.load_file(path.as_path());
    }

    pub fn save(&self) {
        let file_name = self.file_name();

        if self.task_list_day.is_empty() {
            log::debug!("{file_name} is empty. Skipping save.");
            let file_name = file_name + ".toml";
            if let Err(err) = remove_file(self.settings.storage_path.as_path().join(file_name.as_str())) {
                if err.kind() != ErrorKind::NotFound {
                    log::warn!("Tried to delete file {file_name} and failed miserably. {err:?}");
                }
            }
        } else {
            let day = self.to_day();
            if let Err(err) = day.to_file(self.settings.storage_path.as_path(), file_name) {
                log::error!("Failed to save file! {err:?}");
            };
        }

        let git_date = git::date_of_most_recent_commit(self.settings.storage_path.as_path());
        log::debug!("git date: {git_date:?}");
        let git_add_result = git::add(self.settings.storage_path.as_path());
        log::debug!("git add output: {git_add_result:?}");
        let git_commit_result = git::commit(self.settings.storage_path.as_path());
        log::debug!("git commit result: {git_commit_result:?}");
    }

    pub fn save_as_template(&self) {
        let file_name = "template".to_string();
        log::debug!("Want to save {file_name}...");

        if self.task_list_day.is_empty() {
            log::debug!("{file_name} is empty. Skipping save.");
            let file_name = file_name + ".toml";
            if let Err(err) = remove_file(self.settings.storage_path.as_path().join(file_name.as_str())) {
                if err.kind() != ErrorKind::NotFound {
                    log::warn!("Tried to delete file {file_name} and failed miserably. {err:?}");
                }
            }
        } else {
            let day = self.to_day();
            if let Err(err) = day.to_file(self.settings.storage_path.as_path(), file_name) {
                log::error!("Failed to save file! {err:?}");
            };
        }
    }

    pub fn save_global(&self) {
        let file_name = "global".to_string();
        log::debug!("Want to save {file_name}...");

        let day = self.to_day_global();
        if let Err(err) = day.to_file(self.settings.storage_path.as_path(), file_name) {
            log::error!("Failed to save file! {err:?}");
        };
    }

    pub fn go_to_next_day(&mut self) {
        let Some(next) = self.selected_date.checked_add_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_add_days(1)");
            return;
        };
        self.save();
        self.selected_date = next;
        self.load_selected_day();
        self.on_selection_change();
    }

    pub fn go_to_previous_day(&mut self) {
        let Some(next) = self.selected_date.checked_sub_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_sub_days(1)");
            return;
        };
        self.save();
        self.selected_date = next;
        self.load_selected_day();
        self.on_selection_change();
    }

    pub fn go_to_today(&mut self) {
        let next = Local::now().date_naive();
        self.save();
        self.selected_date = next;
        self.load_selected_day();
        self.on_selection_change();
    }

    pub fn copy_previous_day_todo_and_doing(&mut self) {
        let Some(previous) = self.selected_date.checked_sub_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_sub_days(1)");
            return;
        };

        let file_name = previous.format("%Y-%m-%d").to_string();
        let path = self.settings.storage_path.join(file_name + ".toml");
        let day = Day::from_file_or_default(path.as_path());

        for task in day.tasks {
            if task.text.trim().is_empty() {
                continue;
            }

            let tasks_by_status = if task.status == TaskStatus::ToDo {
                &mut self.task_list_day.status_lists_mut()[0]
            } else if task.status == TaskStatus::Doing {
                &mut self.task_list_day.status_lists_mut()[1]
            } else {
                continue;
            };

            let Some(project) = tasks_by_status
                .tasks_by_project
                .iter_mut()
                .find(|p| p.project_id == task.project_id)
            else {
                log::warn!("Could not find project with id {}", task.project_id);
                continue;
            };

            if project.tasks.iter().any(|t| t.as_str() == task.text.as_str()) {
                continue;
            }

            let new_task = EditableTask::new(task.text, task.details.unwrap_or_default());
            let empty_index = project
                .tasks
                .iter()
                .enumerate()
                .rfind(|(_, e)| e.is_empty())
                .map(|(i, _)| i);

            if let Some(empty_index) = empty_index {
                project.tasks.insert(empty_index, new_task);
            } else {
                project.tasks.push(new_task);
            }
        }
    }

    pub fn set_filter(&mut self, project_index: Option<usize>) {
        // Ignore filters that point past the number of visible projects, so e.g.
        // pressing the "filter project 4" key with fewer projects is a no-op
        // rather than an out-of-bounds panic.
        if let Some(i) = project_index
            && i >= self.settings.visible_projects().len()
        {
            return;
        }
        self.project_filter = project_index;
        self.task_list_day.set_filter(project_index);
        self.task_list_global.set_filter(project_index);
        self.on_selection_change();
    }

    pub fn selected_status_mut(&mut self) -> &mut TaskListForStatus {
        self.task_list_day.selected_status_mut()
    }

    pub fn selected_task_mut(&mut self) -> &mut EditableTask {
        self.selected_status_mut().selected_project_mut().selected_task_mut()
    }

    /// What is being edited, if anything.
    pub fn editing(&self) -> Option<EditTarget> {
        if self.task_details.is_editing() {
            Some(EditTarget::TaskDetails)
        } else if self.task_list_day.is_editing() || self.task_list_global.is_editing() {
            Some(EditTarget::TaskTitle)
        } else {
            None
        }
    }

    /// Updates the text of whatever is being edited. Called by the text editor widget on every change.
    pub fn set_editing_text(&mut self, text: String) {
        match self.editing() {
            Some(EditTarget::TaskDetails) => self.task_details.set_description(&text),
            Some(EditTarget::TaskTitle) => {
                if let Some(task) = self.task_list_day.editing_task_mut() {
                    task.set_title(text);
                    // Keeps the title in the details panel up to date.
                    self.on_selection_change();
                } else if let Some(task) = self.task_list_global.editing_task_mut() {
                    task.set_title(text);
                }
            }
            None => {}
        }
    }

    pub(super) fn on_selection_change(&mut self) {
        let task = self.task_list_day.selected_status().selected_task();
        self.task_details.set_title(task.title());
        self.task_details.set_description(task.description());
    }

    pub fn task_list_day(&self) -> &TaskList {
        &self.task_list_day
    }

    pub fn task_list_global(&self) -> &TaskList {
        &self.task_list_global
    }

    pub fn task_details(&self) -> &TaskDetails {
        &self.task_details
    }

    pub fn right_panel(&self) -> &RightPanel {
        &self.right_panel
    }

    pub fn selected_panel(&self) -> Panel {
        if self.selected_panel == 0 {
            Panel::Left
        } else {
            Panel::Right
        }
    }

    /// Confirms whatever is being edited, if anything.
    pub fn finish_editing(&mut self) {
        match self.editing() {
            Some(EditTarget::TaskTitle) => self.on_actions(vec![Action::Text(TextAction::ConfirmEdit)]),
            Some(EditTarget::TaskDetails) => self.on_actions(vec![Action::Text(TextAction::ConfirmEditMultiLine)]),
            None => {}
        }
    }

    /// Focuses a panel, e.g. when clicking on it.
    pub fn focus_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Left => self.selected_panel = 0,
            Panel::Right if self.right_panel != RightPanel::Nothing => self.selected_panel = 1,
            Panel::Right => {}
        }
    }

    /// Selects a task with the mouse. With `edit`, also starts editing it (double click).
    pub fn click_task(&mut self, panel: Panel, status: usize, project: usize, task: usize, edit: bool) {
        self.finish_editing();

        let list = match panel {
            Panel::Left => &mut self.task_list_day,
            Panel::Right if self.right_panel == RightPanel::GlobalTaskList => &mut self.task_list_global,
            Panel::Right => return,
        };

        if !list.select(status, project, task) {
            return;
        }
        if edit {
            list.selected_status_mut()
                .selected_project_mut()
                .selected_task_mut()
                .set_is_editing(true);
        }

        self.focus_panel(panel);
        self.on_selection_change();
    }

    /// Clicking on the description in the details panel starts editing it.
    pub fn click_details(&mut self) {
        if self.right_panel != RightPanel::TaskDetails || self.task_details.is_editing() {
            return;
        }
        self.finish_editing();
        self.focus_panel(Panel::Right);
        self.task_details
            .on_actions(vec![Action::Text(TextAction::EnterEditMode)]);
    }
}

/// What kind of text is being edited.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EditTarget {
    /// The title of a task. Single line.
    TaskTitle,
    /// The description of a task, in the details panel. Multiple lines.
    TaskDetails,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Panel {
    Left,
    Right,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RightPanel {
    Nothing,
    GlobalTaskList,
    TaskDetails,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_selects_and_double_click_edits() {
        let mut root = Root::new(Settings::default());

        // Status 0 (TODO), project 1, task 0.
        root.click_task(Panel::Left, 0, 1, 0, false);
        assert_eq!(root.task_list_day.selected_status_index(), 0);
        assert_eq!(root.task_list_day.selected_status().selected_project, 1);
        assert_eq!(root.editing(), None);

        // Status 2 (DONE), project 2, task 0, double click.
        root.click_task(Panel::Left, 2, 2, 0, true);
        assert_eq!(root.task_list_day.selected_status_index(), 2);
        assert_eq!(root.task_list_day.selected_status().selected_project, 2);
        assert_eq!(root.editing(), Some(EditTarget::TaskTitle));

        root.set_editing_text("hola".to_string());
        // Clicking elsewhere confirms the edit.
        root.click_task(Panel::Left, 0, 0, 0, false);
        assert_eq!(root.editing(), None);
        assert_eq!(
            root.task_list_day.status_lists()[2].tasks_by_project[2].tasks[0].title(),
            "hola"
        );
    }

    #[test]
    fn clicking_the_right_panel_does_nothing_when_it_is_hidden() {
        let mut root = Root::new(Settings::default());
        root.click_task(Panel::Right, 1, 0, 0, false);
        assert_eq!(root.selected_panel(), Panel::Left);
        assert_eq!(root.task_list_global.selected_status_index(), 0);
    }

    #[test]
    fn out_of_range_clicks_are_ignored() {
        let mut root = Root::new(Settings::default());
        root.click_task(Panel::Left, 9, 9, 9, true);
        assert_eq!(root.task_list_day.selected_status_index(), 0);
        assert_eq!(root.editing(), None);
    }
}
