use super::editable_task::EditableTask;
use crate::action::{Action, OnActions, TaskAction, TextAction};

impl OnActions<Action, bool> for &mut EditableTask {
    fn on_actions(self, actions: Vec<Action>) -> bool {
        for action in actions {
            match action {
                Action::Task(TaskAction::EditTitle) if !self.is_editing => {
                    self.set_is_editing(true);
                    return true;
                }
                Action::Text(TextAction::ConfirmEdit) if self.is_editing => {
                    self.set_is_editing(false);
                    return true;
                }
                Action::Text(TextAction::CancelEdit) if self.is_editing => {
                    self.cancel_editing();
                    return true;
                }
                _ => {}
            };
        }

        false
    }
}
