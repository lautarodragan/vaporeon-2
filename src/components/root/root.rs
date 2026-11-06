use std::{fs::remove_file, io::ErrorKind};

use chrono::{Days, Local, NaiveDate};

use super::{
    files::{DataFile, FileSnapshot, Message},
    history::History,
};
use crate::{
    action::{Action, OnActions, TextAction},
    components::{
        EditableTask,
        task_details::TaskDetails,
        task_list::TaskList,
        task_list_for_status::TaskListForStatus,
    },
    git,
    models::Day,
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
    pub(super) project_filter: Option<usize>,
    /// What we know about each file on disk. See [`DataFile`].
    pub(super) snapshots: [FileSnapshot; 2],
    /// Messages for the user, waiting to be shown.
    pub(super) messages: Vec<Message>,
    /// For undo and redo.
    pub(super) history: History,
    /// Whether a day was just loaded (see [`Root::take_day_loaded`]).
    pub(super) day_loaded: bool,
    /// Whether something was committed (see [`Root::take_committed`]).
    pub(super) committed: bool,
    /// Days for which the user said no to copying the unfinished tasks of a previous day.
    pub(super) migration_declined: std::collections::HashSet<NaiveDate>,
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
            project_filter: None,
            right_panel: RightPanel::Nothing,
            snapshots: Default::default(),
            messages: vec![],
            history: History::default(),
            day_loaded: false,
            committed: false,
            migration_declined: Default::default(),
        }
    }

    pub(super) fn file_name(&self) -> String {
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
        let path = self.file_path(DataFile::Day);
        self.task_list_day.load_file(path.as_path());
        self.take_snapshot(DataFile::Day);
        self.day_loaded = true;
    }

    pub fn load_from_template(&mut self) {
        let file_name = "template".to_string();
        let path = self.settings.storage_path.join(file_name + ".toml");
        self.task_list_day.load_file(path.as_path());
    }

    pub fn load_global(&mut self) {
        let path = self.file_path(DataFile::Global);
        self.task_list_global.load_file(path.as_path());
        self.take_snapshot(DataFile::Global);
    }

    /// Saves a file, if it has unsaved changes. Doesn't commit: see [`Root::commit`].
    pub fn save_file(&mut self, file: DataFile) -> SaveResult {
        // Not writing an unchanged file keeps its modification time, so other programs that have it open
        // (e.g. an editor) don't think it changed, and there's nothing to commit.
        if !self.is_dirty(file) {
            return SaveResult::Unchanged;
        }
        let result = match file {
            DataFile::Day => self.write_day(),
            DataFile::Global => self.write_global(),
        };
        self.take_snapshot(file);
        result
    }

    /// Saves the global list and the selected day, if they have unsaved changes, and commits if anything was saved.
    pub fn save_all(&mut self) -> SaveResult {
        // Global first, then the day, and a single commit for both.
        let global = self.save_file(DataFile::Global);
        let day = self.save_file(DataFile::Day);
        if global == SaveResult::Saved || day == SaveResult::Saved {
            self.commit();
        }
        global.and(day)
    }

    fn write_day(&mut self) -> SaveResult {
        let file_name = self.file_name();

        if self.task_list_day.is_empty() {
            log::debug!("{file_name} is empty. Deleting it.");
            let file_name = file_name + ".toml";
            if let Err(err) = remove_file(self.settings.storage_path.as_path().join(file_name.as_str()))
                && err.kind() != ErrorKind::NotFound
            {
                self.error(format!("Couldn't delete the empty {file_name}: {err}"));
                return SaveResult::Failed;
            }
        } else {
            let day = self.to_day();
            if let Err(err) = day.to_file(self.settings.storage_path.as_path(), file_name.clone()) {
                self.error(format!("Couldn't save {file_name}.toml: {err:?}"));
                return SaveResult::Failed;
            };
        }
        SaveResult::Saved
    }

    fn write_global(&mut self) -> SaveResult {
        let day = self.to_day_global();
        match day.to_file(self.settings.storage_path.as_path(), "global".to_string()) {
            Ok(()) => SaveResult::Saved,
            Err(err) => {
                self.error(format!("Couldn't save global.toml: {err:?}"));
                SaveResult::Failed
            }
        }
    }

    /// Commits the changes in `storage_path`, if it's a git repository.
    pub fn commit(&mut self) {
        let git_started_at = std::time::Instant::now();
        match git::commit_all(self.settings.storage_path.as_path()) {
            Ok(committed) => self.committed |= committed,
            Err(err) => log::debug!("Not committed: {err:?}"),
        }
        log::debug!("git took {} ms", git_started_at.elapsed().as_millis());
    }

    /// Whether something was committed since the last call (see [`Root::commit`]). Resets the flag.
    pub fn take_committed(&mut self) -> bool {
        std::mem::take(&mut self.committed)
    }

    pub fn storage_path(&self) -> &std::path::Path {
        &self.settings.storage_path
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

    pub fn selected_date(&self) -> NaiveDate {
        self.selected_date
    }

    /// Saves the current day, and loads another one.
    pub fn go_to_date(&mut self, date: NaiveDate) {
        if self.save_file(DataFile::Day) == SaveResult::Saved {
            self.commit();
        }
        self.selected_date = date;
        self.load_selected_day();
        self.on_selection_change();
    }

    pub fn go_to_next_day(&mut self) {
        let Some(next) = self.selected_date.checked_add_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_add_days(1)");
            return;
        };
        self.go_to_date(next);
    }

    pub fn go_to_previous_day(&mut self) {
        let Some(previous) = self.selected_date.checked_sub_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_sub_days(1)");
            return;
        };
        self.go_to_date(previous);
    }

    pub fn go_to_today(&mut self) {
        self.go_to_date(Local::now().date_naive());
    }

    pub fn copy_previous_day_todo_and_doing(&mut self) {
        let Some(previous) = self.selected_date.checked_sub_days(Days::new(1)) else {
            log::error!("Failed to selected_date.checked_sub_days(1)");
            return;
        };
        self.copy_unfinished_from(previous);
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

    fn list_mut(&mut self, panel: Panel) -> Option<&mut TaskList> {
        match panel {
            Panel::Left => Some(&mut self.task_list_day),
            Panel::Right if self.right_panel == RightPanel::GlobalTaskList => Some(&mut self.task_list_global),
            Panel::Right => None,
        }
    }

    pub fn list(&self, panel: Panel) -> Option<&TaskList> {
        match panel {
            Panel::Left => Some(&self.task_list_day),
            Panel::Right if self.right_panel == RightPanel::GlobalTaskList => Some(&self.task_list_global),
            Panel::Right => None,
        }
    }

    /// Moves a task (drag and drop). It can go to another position in the same project, to another project or status,
    /// or to the other panel.
    ///
    /// `to.task` is the position *between rows* where it was dropped, as seen on screen before moving: 0 is before
    /// the first task, 1 is between the first and second task, etc.
    ///
    /// Returns false if nothing was moved (e.g. empty task, or positions out of range).
    pub fn move_task(&mut self, from: TaskPosition, mut to: TaskPosition) -> bool {
        self.finish_editing();

        if !self
            .list(to.panel)
            .is_some_and(|list| list.has_project(to.status, to.project))
        {
            return false;
        }

        let Some(source) = self.list_mut(from.panel) else {
            return false;
        };
        let tasks_before = source
            .status_lists()
            .get(from.status)
            .and_then(|list| list.tasks_by_project.get(from.project))
            .map(|project| project.tasks.len())
            .unwrap_or(0);
        let Some((title, description)) = source.take_task(from.status, from.project, from.task) else {
            return false;
        };

        // If the task was removed from above the drop position, everything below it moved up one row.
        let same_project = from.panel == to.panel && from.status == to.status && from.project == to.project;
        if same_project && tasks_before > 1 && from.task < to.task {
            to.task -= 1;
        }

        if let Some(target) = self.list_mut(to.panel) {
            target.insert_task(to.status, to.project, to.task, title, description);
        }

        self.focus_panel(to.panel);
        self.on_selection_change();
        true
    }

    /// Adds a task with text dropped from another application. The first line is the title, the rest the description.
    pub fn insert_dropped_text(&mut self, to: TaskPosition, text: &str) -> bool {
        self.finish_editing();

        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        let (title, description) = text.split_once('\n').unwrap_or((text, ""));

        let Some(target) = self.list_mut(to.panel) else {
            return false;
        };
        if !target.insert_task(
            to.status,
            to.project,
            to.task,
            title.trim().to_string(),
            description.trim().to_string(),
        ) {
            return false;
        }

        self.focus_panel(to.panel);
        self.on_selection_change();
        true
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

/// What happened when saving.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SaveResult {
    Saved,
    /// There were no unsaved changes, so nothing was written.
    Unchanged,
    /// The error was reported as a message.
    Failed,
}

impl SaveResult {
    /// The result of saving two things.
    fn and(self, other: SaveResult) -> SaveResult {
        use SaveResult::*;
        match (self, other) {
            (Failed, _) | (_, Failed) => Failed,
            (Saved, _) | (_, Saved) => Saved,
            (Unchanged, Unchanged) => Unchanged,
        }
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

/// The position of a task in one of the task lists.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaskPosition {
    pub panel: Panel,
    pub status: usize,
    pub project: usize,
    pub task: usize,
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

    fn root_with_tasks() -> Root {
        let mut root = Root::new(Settings::default());
        // TODO, project 0: a, b, c. DONE, project 1: x.
        for (status, project, title) in [(0, 0, "c"), (0, 0, "b"), (0, 0, "a"), (2, 1, "x")] {
            root.task_list_day
                .insert_task(status, project, 0, title.to_string(), String::new());
        }
        root
    }

    fn titles(root: &Root, status: usize, project: usize) -> Vec<String> {
        root.task_list_day.status_lists()[status].tasks_by_project[project]
            .tasks
            .iter()
            .map(|t| t.title().to_string())
            .collect()
    }

    fn pos(status: usize, project: usize, task: usize) -> TaskPosition {
        TaskPosition {
            panel: Panel::Left,
            status,
            project,
            task,
        }
    }

    #[test]
    fn move_down_and_up_within_a_project() {
        let mut root = root_with_tasks();
        // Drag "a" to between "b" and "c".
        assert!(root.move_task(pos(0, 0, 0), pos(0, 0, 2)));
        assert_eq!(titles(&root, 0, 0), ["b", "a", "c"]);
        // Drag "c" to the top.
        assert!(root.move_task(pos(0, 0, 2), pos(0, 0, 0)));
        assert_eq!(titles(&root, 0, 0), ["c", "b", "a"]);
        // Dropping right where it was does nothing.
        assert!(root.move_task(pos(0, 0, 1), pos(0, 0, 2)));
        assert_eq!(titles(&root, 0, 0), ["c", "b", "a"]);
        // The moved task is selected.
        assert_eq!(root.task_list_day.selected_status().selected_task().title(), "b");
    }

    #[test]
    fn move_to_another_status_and_project() {
        let mut root = root_with_tasks();
        assert!(root.move_task(pos(0, 0, 1), pos(2, 1, 1)));
        assert_eq!(titles(&root, 0, 0), ["a", "c"]);
        assert_eq!(titles(&root, 2, 1), ["x", "b"]);
        // Into an empty project: replaces its empty placeholder task.
        assert!(root.move_task(pos(0, 0, 0), pos(1, 2, 0)));
        assert_eq!(titles(&root, 1, 2), ["a"]);
        // Moving the last task of a project leaves an empty placeholder behind.
        assert!(root.move_task(pos(1, 2, 0), pos(0, 0, 0)));
        assert_eq!(titles(&root, 1, 2), [""]);
        assert_eq!(titles(&root, 0, 0), ["a", "c"]);
    }

    #[test]
    fn empty_tasks_and_invalid_targets_are_not_moved() {
        let mut root = root_with_tasks();
        assert!(!root.move_task(pos(1, 0, 0), pos(0, 0, 0)));
        assert!(!root.move_task(pos(0, 0, 0), pos(9, 0, 0)));
        let to_hidden_panel = TaskPosition {
            panel: Panel::Right,
            ..pos(0, 0, 0)
        };
        assert!(!root.move_task(pos(0, 0, 0), to_hidden_panel));
        assert_eq!(titles(&root, 0, 0), ["a", "b", "c"]);
    }

    #[test]
    fn dropped_text_becomes_a_task() {
        let mut root = root_with_tasks();
        assert!(root.insert_dropped_text(pos(0, 0, 1), "  hola\nmundo\n "));
        assert_eq!(titles(&root, 0, 0), ["a", "hola", "b", "c"]);
        assert_eq!(
            root.task_list_day.status_lists()[0].tasks_by_project[0].tasks[1].description(),
            "mundo"
        );
        assert!(!root.insert_dropped_text(pos(0, 0, 0), "   "));
    }

    #[test]
    fn out_of_range_clicks_are_ignored() {
        let mut root = Root::new(Settings::default());
        root.click_task(Panel::Left, 9, 9, 9, true);
        assert_eq!(root.task_list_day.selected_status_index(), 0);
        assert_eq!(root.editing(), None);
    }
}
