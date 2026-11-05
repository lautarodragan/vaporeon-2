//! "Migrating" unfinished tasks, bullet-journal style: when starting a new day, offer to copy the tasks that were left
//! to do (or doing) the last day there were tasks.

use chrono::{Days, Local, NaiveDate};

use super::Root;
use crate::{
    components::EditableTask,
    models::{Day, Task, TaskStatus},
};

/// How far back to look for the last day with tasks.
const MAX_DAYS_BACK: u64 = 14;

/// An offer to copy unfinished tasks from a previous day.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MigrationOffer {
    pub from: NaiveDate,
    pub count: usize,
}

impl Root {
    fn day_file(&self, date: NaiveDate) -> Day {
        let path = self
            .settings
            .storage_path
            .join(date.format("%Y-%m-%d").to_string() + ".toml");
        Day::from_file_or_default(&path)
    }

    fn is_unfinished(&self, task: &Task) -> bool {
        matches!(task.status, TaskStatus::ToDo | TaskStatus::Doing)
            && !task.text.trim().is_empty()
            && self.settings.visible_projects().iter().any(|p| p.id == task.project_id)
    }

    /// Whether a day was just loaded. Resets the flag.
    pub fn take_day_loaded(&mut self) -> bool {
        std::mem::take(&mut self.day_loaded)
    }

    /// If the selected day is today, and it's empty, and the last day with tasks (looking back up to two weeks) has
    /// unfinished ones, offers to copy them. Unless the offer was turned down already.
    pub fn migration_offer(&self) -> Option<MigrationOffer> {
        let today = Local::now().date_naive();
        if self.selected_date != today || !self.task_list_day.is_empty() || self.migration_declined.contains(&today) {
            return None;
        }

        // The last day with any tasks, finished or not.
        let (from, day) = (1..=MAX_DAYS_BACK)
            .filter_map(|n| today.checked_sub_days(Days::new(n)))
            .map(|date| (date, self.day_file(date)))
            .find(|(_, day)| day.tasks.iter().any(|t| !t.text.trim().is_empty()))?;

        let count = day.tasks.iter().filter(|t| self.is_unfinished(t)).count();
        (count > 0).then_some(MigrationOffer { from, count })
    }

    /// Don't offer to copy unfinished tasks to the selected day again (until the app is restarted).
    pub fn decline_migration(&mut self) {
        self.migration_declined.insert(self.selected_date);
    }

    /// Copies the unfinished (to do, doing) tasks of a day to the selected day, to the same status and project.
    /// Tasks that are already there (same title) aren't copied again. Returns how many tasks were copied.
    pub fn copy_unfinished_from(&mut self, date: NaiveDate) -> usize {
        let mut copied = 0;

        for task in self.day_file(date).tasks {
            if !self.is_unfinished(&task) {
                continue;
            }

            let status = if task.status == TaskStatus::ToDo { 0 } else { 1 };
            let Some(project) = self.task_list_day.status_lists_mut()[status]
                .tasks_by_project
                .iter_mut()
                .find(|p| p.project_id == task.project_id)
            else {
                continue;
            };

            if project.tasks.iter().any(|t| t.as_str() == task.text.as_str()) {
                continue;
            }

            let new_task = EditableTask::new(task.text, task.details.unwrap_or_default());
            // Before the empty task at the end of the project, if there's one, so it stays at the end.
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
            copied += 1;
        }

        self.on_selection_change();
        copied
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{components::root::DataFile, settings::Settings};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaporeon-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn task(text: &str, status: TaskStatus) -> Task {
        Task {
            text: text.to_string(),
            project_id: 0,
            status,
            details: None,
        }
    }

    fn write_day(dir: &std::path::Path, days_ago: u64, tasks: Vec<Task>) {
        let date = Local::now().date_naive() - Days::new(days_ago);
        let day = Day { date_time: date, tasks };
        day.to_file(dir, date.format("%Y-%m-%d").to_string()).unwrap();
    }

    fn root_in(dir: &std::path::Path) -> Root {
        let settings = Settings {
            storage_path: dir.to_path_buf(),
            ..Settings::default()
        };
        let mut root = Root::new(settings);
        root.load_selected_day();
        root
    }

    #[test]
    fn offers_unfinished_tasks_of_the_last_day_with_tasks() {
        let dir = temp_dir("migration-offer");
        // Yesterday and the day before were empty (e.g. a weekend).
        write_day(&dir, 3, vec![
            task("a", TaskStatus::ToDo),
            task("b", TaskStatus::Doing),
            task("c", TaskStatus::Done),
            task("", TaskStatus::ToDo),
        ]);
        let mut root = root_in(&dir);

        let offer = root.migration_offer().unwrap();
        assert_eq!(offer.count, 2);
        assert_eq!(offer.from, Local::now().date_naive() - Days::new(3));

        assert_eq!(root.copy_unfinished_from(offer.from), 2);
        assert!(root.is_dirty(DataFile::Day));
        // Today isn't empty anymore.
        assert_eq!(root.migration_offer(), None);
        // Copying again doesn't duplicate them.
        assert_eq!(root.copy_unfinished_from(offer.from), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_offer_when_everything_was_done_or_declined() {
        let dir = temp_dir("migration-none");
        write_day(&dir, 1, vec![task("done", TaskStatus::Done)]);
        let mut root = root_in(&dir);
        assert_eq!(root.migration_offer(), None);

        write_day(&dir, 1, vec![task("todo", TaskStatus::ToDo)]);
        assert!(root.migration_offer().is_some());
        root.decline_migration();
        assert_eq!(root.migration_offer(), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
