//! Keeping track of the files on disk: whether they changed outside of Vaporeon, whether there are unsaved changes,
//! and what to do when both happen at the same time.

use std::{fs::read_to_string, path::PathBuf};

use chrono::Local;

use super::{Root, root::Event};

/// One of the files the UI shows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DataFile {
    /// The file of the selected day.
    Day,
    /// The global task list.
    Global,
}

impl DataFile {
    pub const ALL: [DataFile; 2] = [DataFile::Day, DataFile::Global];

    fn index(self) -> usize {
        self as usize
    }
}

/// What we know about a file: its contents on disk, and what the model looked like, as of the last time we read or
/// wrote it.
#[derive(Debug, Default, Clone)]
pub struct FileSnapshot {
    /// The contents of the file on disk. None if it doesn't exist.
    disk: Option<String>,
    /// The model, serialized. Comparing it with the current model tells whether there are unsaved changes.
    model: String,
}

/// What happened when checking whether a file changed outside of Vaporeon.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExternalChange {
    /// The file is just like we left it (e.g. the change was us saving it).
    None,
    /// The file changed, and there were no unsaved changes, so it was reloaded.
    Reloaded,
    /// The file changed, but there are unsaved changes. The user has to choose what to do.
    Conflict,
}

/// What to do when a file changed outside of Vaporeon and there are unsaved changes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Discard the unsaved changes, and load the file.
    Reload,
    /// Keep the unsaved changes. Saving will overwrite the file.
    KeepMine,
    /// Save the unsaved changes to a backup file, and load the file.
    BackupAndReload,
}

/// A message for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub is_error: bool,
}

impl Root {
    pub fn file_path(&self, file: DataFile) -> PathBuf {
        let name = match file {
            DataFile::Day => self.file_name(),
            DataFile::Global => "global".to_string(),
        };
        self.settings.storage_path.join(name + ".toml")
    }

    pub fn file_display_name(&self, file: DataFile) -> String {
        self.file_path(file)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn serialize(&self, file: DataFile) -> String {
        let day = match file {
            DataFile::Day => self.to_day(),
            DataFile::Global => self.to_day_global(),
        };
        crate::format::day_to_string(&day)
    }

    /// Remembers the file as it is now on disk, and the model as it is now. Called after loading or saving it.
    pub(super) fn take_snapshot(&mut self, file: DataFile) {
        self.snapshots[file.index()] = FileSnapshot {
            disk: read_to_string(self.file_path(file)).ok(),
            model: self.serialize(file),
        };
    }

    /// Whether the model changed since the file was last loaded or saved.
    pub fn is_dirty(&self, file: DataFile) -> bool {
        self.serialize(file) != self.snapshots[file.index()].model
    }

    /// Whether something is being edited in the part of the UI that shows this file.
    fn is_editing(&self, file: DataFile) -> bool {
        match file {
            DataFile::Day => self.task_list_day.is_editing() || self.task_details.is_editing(),
            DataFile::Global => self.task_list_global.is_editing(),
        }
    }

    fn changed_on_disk(&self, file: DataFile) -> bool {
        read_to_string(self.file_path(file)).ok() != self.snapshots[file.index()].disk
    }

    /// Checks whether a file changed outside of Vaporeon. If it did, and there are no unsaved changes, reloads it.
    pub fn check_external_change(&mut self, file: DataFile) -> ExternalChange {
        if !self.changed_on_disk(file) {
            return ExternalChange::None;
        }

        if self.is_dirty(file) || self.is_editing(file) {
            return ExternalChange::Conflict;
        }

        self.reload(file);
        self.info(format!("{} changed on disk. Reloaded.", self.file_display_name(file)));
        ExternalChange::Reloaded
    }

    pub fn resolve_conflict(&mut self, file: DataFile, resolution: Resolution) {
        let name = self.file_display_name(file);
        match resolution {
            Resolution::Reload => {
                self.reload(file);
                self.info(format!("Reloaded {name}."));
            }
            Resolution::KeepMine => {
                // Acknowledge the version on disk, so we don't ask again unless it changes again.
                self.snapshots[file.index()].disk = read_to_string(self.file_path(file)).ok();
                self.info(format!(
                    "Keeping your version of {name}. Saving will overwrite the file."
                ));
            }
            Resolution::BackupAndReload => match self.backup(file) {
                Ok(backup) => {
                    self.reload(file);
                    self.info(format!("Your version was saved to {backup}. Reloaded {name}."));
                }
                Err(err) => self.error(format!(
                    "Couldn't save a backup of {name} ({err}). Nothing was changed."
                )),
            },
        }
    }

    /// Saves the current (unsaved) version of a file next to it, as `<name>.toml.bak-<time>`. Doesn't touch the
    /// file itself. The backup isn't committed to git (only `*.toml` files are).
    pub fn backup(&self, file: DataFile) -> std::io::Result<String> {
        let path = self.file_path(file);
        let name = format!(
            "{}.bak-{}",
            self.file_display_name(file),
            Local::now().format("%Y%m%d-%H%M%S")
        );
        std::fs::create_dir_all(&self.settings.storage_path)?;
        std::fs::write(path.with_file_name(&name), self.serialize(file))?;
        Ok(name)
    }

    fn reload(&mut self, file: DataFile) {
        // Whatever was being edited belongs to the old version.
        if self.is_editing(file) {
            self.cancel_editing();
        }
        match file {
            DataFile::Day => self.load_selected_day(),
            DataFile::Global => self.load_global(),
        }
        self.on_selection_change();
        // Undoing shouldn't bring back what was there before reloading.
        self.reset_history();
    }

    pub fn info(&mut self, text: String) {
        log::info!("{text}");
        self.events.push(Event::Message(Message { text, is_error: false }));
    }

    pub fn error(&mut self, text: String) {
        log::error!("{text}");
        self.events.push(Event::Message(Message { text, is_error: true }));
    }

    /// Things the UI should know about, since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn root_in(dir: &std::path::Path) -> Root {
        let settings = Settings {
            storage_path: dir.to_path_buf(),
            ..Settings::default()
        };
        let mut root = Root::new(settings);
        root.load_selected_day();
        root.load_global();
        root
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaporeon-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add_task(root: &mut Root, title: &str) {
        root.task_list_day
            .insert_task(0, 0, 0, title.to_string(), String::new());
    }

    fn day_titles(root: &Root) -> Vec<String> {
        root.task_list_day.status_lists()[0].tasks_by_project[0]
            .tasks
            .iter()
            .map(|t| t.title().to_string())
            .collect()
    }

    /// Writes the day file as another program would: with the given tasks (in TODO, first project).
    fn write_externally(root: &Root, titles: &[&str]) {
        let mut other = Root::new(root.settings.clone());
        for title in titles.iter().rev() {
            other
                .task_list_day
                .insert_task(0, 0, 0, title.to_string(), String::new());
        }
        other.selected_date = root.selected_date;
        std::fs::write(root.file_path(DataFile::Day), toml::to_string(&other.to_day()).unwrap()).unwrap();
    }

    #[test]
    fn own_saves_are_not_external_changes() {
        let dir = temp_dir("own-saves");
        let mut root = root_in(&dir);
        assert!(!root.is_dirty(DataFile::Day));

        add_task(&mut root, "a");
        assert!(root.is_dirty(DataFile::Day));

        root.save_all();
        assert!(!root.is_dirty(DataFile::Day));
        assert_eq!(root.check_external_change(DataFile::Day), ExternalChange::None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_files_are_not_written() {
        let dir = temp_dir("unchanged");
        let mut root = root_in(&dir);
        add_task(&mut root, "a");
        assert_eq!(root.save_all(), crate::components::root::SaveResult::Saved);

        // Make the file different from what Vaporeon would write, without changing what it means
        // (as if formatted by hand): saving again must not touch it.
        let path = root.file_path(DataFile::Day);
        let hand_formatted = format!("# my comment\n{}", std::fs::read_to_string(&path).unwrap());
        std::fs::write(&path, &hand_formatted).unwrap();
        root.take_snapshot(DataFile::Day);

        assert_eq!(root.save_all(), crate::components::root::SaveResult::Unchanged);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), hand_formatted);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn external_changes_reload_when_there_are_no_unsaved_changes() {
        let dir = temp_dir("reload");
        let mut root = root_in(&dir);
        add_task(&mut root, "a");
        root.save_all();

        write_externally(&root, &["a", "b"]);
        assert_eq!(root.check_external_change(DataFile::Day), ExternalChange::Reloaded);
        assert_eq!(day_titles(&root), ["a", "b"]);
        assert!(!root.is_dirty(DataFile::Day));
        let messages = root
            .take_events()
            .into_iter()
            .filter(|event| matches!(event, Event::Message(_)))
            .count();
        assert_eq!(messages, 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn external_changes_with_unsaved_changes_are_conflicts() {
        let dir = temp_dir("conflict");
        let mut root = root_in(&dir);
        add_task(&mut root, "a");
        root.save_all();
        add_task(&mut root, "mine");
        write_externally(&root, &["a", "theirs"]);

        assert_eq!(root.check_external_change(DataFile::Day), ExternalChange::Conflict);
        assert_eq!(day_titles(&root), ["mine", "a"]);

        // Keep mine: no more conflicts until the file changes again, still unsaved.
        root.resolve_conflict(DataFile::Day, Resolution::KeepMine);
        assert_eq!(root.check_external_change(DataFile::Day), ExternalChange::None);
        assert!(root.is_dirty(DataFile::Day));

        // Backup and reload.
        write_externally(&root, &["a", "theirs", "more"]);
        assert_eq!(root.check_external_change(DataFile::Day), ExternalChange::Conflict);
        root.resolve_conflict(DataFile::Day, Resolution::BackupAndReload);
        assert_eq!(day_titles(&root), ["a", "theirs", "more"]);
        let backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".toml.bak-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(std::fs::read_to_string(backups[0].path()).unwrap().contains("mine"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
