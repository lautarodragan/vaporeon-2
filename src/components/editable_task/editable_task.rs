use crate::models::Task as DayTask;

/// A task in a task list. The text itself is edited by a GTK text widget (see `main.rs`);
/// this only keeps track of whether the task is being edited.
#[derive(Clone, Debug)]
pub struct EditableTask {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) is_editing: bool,
    /// Title before editing started, so the edit can be cancelled.
    pub(super) title_before_editing: Option<String>,
}

impl EditableTask {
    pub fn new(title: String, description: String) -> Self {
        Self {
            title,
            description,
            is_editing: false,
            title_before_editing: None,
        }
    }

    pub fn empty() -> Self {
        Self::new("".to_string(), "".to_string())
    }

    pub fn from_day_task(day_task: &DayTask) -> Self {
        Self::new(day_task.text.clone(), day_task.details.clone().unwrap_or_default())
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    pub fn set_is_editing(&mut self, v: bool) {
        if v && !self.is_editing {
            self.title_before_editing = Some(self.title.clone());
        } else if !v {
            self.title_before_editing = None;
        }
        self.is_editing = v;
    }

    /// Stops editing, restoring the title to what it was before editing started.
    pub fn cancel_editing(&mut self) {
        if let Some(title) = self.title_before_editing.take() {
            self.title = title;
        }
        self.is_editing = false;
    }

    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty()
    }

    pub fn as_str(&self) -> &str {
        self.title.as_str()
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }

    pub fn take_title(&mut self) -> String {
        std::mem::take(&mut self.title)
    }

    pub fn set_title(&mut self, v: String) {
        self.title = v;
    }

    pub fn take_description(&mut self) -> String {
        std::mem::take(&mut self.description)
    }

    pub fn into_parts(self) -> (String, String) {
        (self.title, self.description)
    }

    pub fn clear(&mut self) {
        self.title.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, OnActions, TextAction};

    #[test]
    fn cancel_restores_title() {
        let mut task = EditableTask::new("hola".to_string(), "".to_string());
        assert!(task.on_actions(vec![Action::Text(TextAction::EnterEditMode)]));
        task.set_title("chau".to_string());
        assert!(task.on_actions(vec![Action::Text(TextAction::CancelEdit)]));
        assert_eq!(task.title(), "hola");
        assert!(!task.is_editing());
    }

    #[test]
    fn confirm_keeps_title() {
        let mut task = EditableTask::new("hola".to_string(), "".to_string());
        task.on_actions(vec![Action::Text(TextAction::EnterEditMode)]);
        task.set_title("chau".to_string());
        task.on_actions(vec![Action::Text(TextAction::ConfirmEdit)]);
        assert_eq!(task.title(), "chau");
        assert!(!task.is_editing());
    }
}
