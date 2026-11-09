//! Undo and redo.
//!
//! Instead of recording each kind of change, we take "snapshots" of the content (the tasks of the selected day and
//! of the global list) after every change (see [`Root::checkpoint`]). Undoing goes back to the previous snapshot.
//! This way every change is undoable, including ones added in the future, without each having to support it.

use chrono::NaiveDate;

use super::{Panel, Root};
use crate::models::Day;

/// How many changes can be undone.
const MAX_UNDO: usize = 100;

#[derive(Debug, Clone)]
pub struct Snapshot {
    day: Day,
    global: Day,
    /// Selection (status, project, task) in each list, restored when going back to this snapshot.
    day_selection: (usize, usize, usize),
    global_selection: (usize, usize, usize),
    panel: Panel,
}

impl Snapshot {
    /// Whether the tasks are the same. The selection doesn't count: moving around isn't a change.
    fn same_content(&self, other: &Snapshot) -> bool {
        self.day == other.day && self.global == other.global
    }
}

#[derive(Debug, Default)]
pub struct History {
    /// The day the history is about. It starts over when going to another day.
    date: Option<NaiveDate>,
    /// The content as of the last checkpoint.
    last: Option<Snapshot>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Counts the changes to the content (including undoing and redoing), so others can tell whether it changed
    /// (see [`Root::changes`]).
    changes: u64,
}

impl Root {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            day: self.task_list_day.to_day(),
            global: self.task_list_global.to_day(),
            day_selection: self.task_list_day.selection(),
            global_selection: self.task_list_global.selection(),
            panel: self.selected_panel(),
        }
    }

    /// Records the current content as a step that can be undone, if it changed since the last checkpoint.
    ///
    /// Called after anything that may change the content. Does nothing while editing text, so editing a task (from
    /// starting to edit it to confirming) is a single step.
    pub fn checkpoint(&mut self) {
        if self.editing().is_some() {
            return;
        }

        let current = self.snapshot();
        let Some(last) = &self.history.last else {
            self.reset_history();
            return;
        };
        if self.history.date != Some(self.selected_date) {
            self.reset_history();
            return;
        }

        if current.same_content(last) {
            // Remember where the selection is, so undoing the next change goes back to it.
            self.history.last = Some(current);
            return;
        }

        self.history.changes += 1;
        let last = self.history.last.replace(current).expect("checked above");
        self.history.undo.push(last);
        if self.history.undo.len() > MAX_UNDO {
            self.history.undo.remove(0);
        }
        self.history.redo.clear();
    }

    /// Forgets the undo history. Used when the content is replaced by something else, e.g. reloaded from disk:
    /// undoing shouldn't bring back what was there before.
    pub fn reset_history(&mut self) {
        self.history = History {
            date: Some(self.selected_date),
            last: Some(self.snapshot()),
            undo: vec![],
            redo: vec![],
            changes: self.history.changes,
        };
    }

    /// A number that grows every time the content changes (as of the last checkpoint), e.g. to save some time after
    /// the last change. Moving the selection isn't a change.
    pub fn changes(&self) -> u64 {
        self.history.changes
    }

    /// Undoes the last change. Returns false if there's nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.step(true)
    }

    /// Redoes the last change undone. Returns false if there's nothing to redo.
    pub fn redo(&mut self) -> bool {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> bool {
        if self.editing().is_some() {
            return false;
        }
        // Make sure the latest change is in the history.
        self.checkpoint();

        let (from, to) = if undo {
            (&mut self.history.undo, &mut self.history.redo)
        } else {
            (&mut self.history.redo, &mut self.history.undo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        if let Some(current) = self.history.last.take() {
            to.push(current);
        }

        self.restore(&snapshot);
        self.history.last = Some(snapshot);
        self.history.changes += 1;
        true
    }

    fn restore(&mut self, snapshot: &Snapshot) {
        self.task_list_day.load_day(snapshot.day.clone());
        self.task_list_global.load_day(snapshot.global.clone());
        let (s, p, t) = snapshot.day_selection;
        self.task_list_day.select(s, p, t);
        let (s, p, t) = snapshot.global_selection;
        self.task_list_global.select(s, p, t);
        self.focus_panel(snapshot.panel);
        self.on_selection_change();
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        action::{Action, OnActions, TaskAction, TextAction},
        components::root::{Root, TaskPosition},
        settings::Settings,
    };

    fn titles(root: &Root) -> Vec<String> {
        root.task_list_day.status_lists()[0].tasks_by_project[0]
            .tasks
            .iter()
            .map(|t| t.title().to_string())
            .collect()
    }

    fn add(root: &mut Root, title: &str) {
        let n = root.task_list_day.status_lists()[0].tasks_by_project[0].tasks.len();
        root.task_list_day
            .insert_task(0, 0, n, title.to_string(), String::new());
        root.checkpoint();
    }

    fn new_root() -> Root {
        let mut root = Root::new(Settings::default());
        root.checkpoint();
        root
    }

    #[test]
    fn undo_and_redo() {
        let mut root = new_root();
        add(&mut root, "a");
        add(&mut root, "b");
        assert_eq!(titles(&root), ["a", "b"]);

        assert!(root.undo());
        assert_eq!(titles(&root), ["a"]);
        assert!(root.undo());
        assert_eq!(titles(&root), [""]);
        assert!(!root.undo());

        assert!(root.redo());
        assert_eq!(titles(&root), ["a"]);
        assert!(root.redo());
        assert_eq!(titles(&root), ["a", "b"]);
        assert!(!root.redo());
    }

    #[test]
    fn a_new_change_after_undoing_clears_redo() {
        let mut root = new_root();
        add(&mut root, "a");
        add(&mut root, "b");
        root.undo();
        add(&mut root, "c");
        assert_eq!(titles(&root), ["a", "c"]);
        assert!(!root.redo());
    }

    #[test]
    fn moving_around_is_not_a_change() {
        let mut root = new_root();
        add(&mut root, "a");
        root.task_list_day.select(1, 0, 0);
        root.checkpoint();
        assert!(root.undo());
        assert_eq!(titles(&root), [""]);
        assert!(!root.undo());
    }

    #[test]
    fn editing_a_title_is_a_single_step() {
        let mut root = new_root();
        add(&mut root, "a");
        root.task_list_day.select(0, 0, 0);
        root.on_actions(vec![Action::Task(TaskAction::EditTitle)]);
        for title in ["ab", "abc", "abcd"] {
            root.set_editing_text(title.to_string());
            root.checkpoint(); // Ignored while editing.
        }
        root.on_actions(vec![Action::Text(TextAction::ConfirmEdit)]);
        root.checkpoint();
        assert_eq!(titles(&root), ["abcd"]);

        assert!(root.undo());
        assert_eq!(titles(&root), ["a"]);
    }

    #[test]
    fn deleting_and_moving_can_be_undone() {
        let mut root = new_root();
        add(&mut root, "a");
        add(&mut root, "b");
        root.task_list_day.select(0, 0, 0);
        root.on_actions(vec![Action::Task(TaskAction::Delete)]);
        root.checkpoint();
        assert_eq!(titles(&root), ["b"]);

        let from = TaskPosition {
            panel: crate::components::root::Panel::Left,
            status: 0,
            project: 0,
            task: 0,
        };
        root.move_task(from, TaskPosition { status: 2, ..from });
        root.checkpoint();
        assert_eq!(titles(&root), [""]);

        assert!(root.undo());
        assert_eq!(titles(&root), ["b"]);
        assert!(root.undo());
        assert_eq!(titles(&root), ["a", "b"]);
    }

    #[test]
    fn going_to_another_day_starts_over() {
        let mut root = new_root();
        add(&mut root, "a");
        root.selected_date = root.selected_date.succ_opt().unwrap();
        root.checkpoint();
        assert!(!root.undo());
    }

    #[test]
    fn changes_count_content_changes_but_not_moving_the_selection() {
        let mut root = new_root();
        let start = root.changes();
        add(&mut root, "a");
        add(&mut root, "b");
        assert_eq!(root.changes(), start + 2);

        root.task_list_day.select(0, 0, 0);
        root.checkpoint();
        assert_eq!(root.changes(), start + 2);

        assert!(root.undo());
        assert_eq!(root.changes(), start + 3);
    }
}
