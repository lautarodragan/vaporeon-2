use crate::{
    action::{Action, OnActions, TextAction},
    components::task_details::TaskDetails,
};

#[derive(Debug)]
pub enum TaskDetailsEvent {
    DescriptionConfirmed,
    EditCancelled,
}

impl OnActions<Action, (bool, Option<TaskDetailsEvent>)> for &mut TaskDetails {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TaskDetailsEvent>) {
        for action in actions {
            match action {
                Action::Text(TextAction::EnterEditMode) if !self.is_editing => {
                    self.is_editing = true;
                    return (true, None);
                }
                Action::Text(TextAction::ConfirmEditMultiLine) if self.is_editing => {
                    self.is_editing = false;
                    return (true, Some(TaskDetailsEvent::DescriptionConfirmed));
                }
                Action::Text(TextAction::CancelEdit) if self.is_editing => {
                    self.is_editing = false;
                    return (true, Some(TaskDetailsEvent::DescriptionConfirmed));
                }
                _ => {}
            }
        }
        (self.is_editing, None)
    }
}
