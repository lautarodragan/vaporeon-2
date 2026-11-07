use crate::{
    action::{Action, OnActions, TaskAction, TextAction},
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
                Action::Task(TaskAction::EditDescription) if !self.is_editing => {
                    self.start_editing();
                    return (true, None);
                }
                Action::Text(TextAction::ConfirmEditMultiLine) if self.is_editing => {
                    self.is_editing = false;
                    return (true, Some(TaskDetailsEvent::DescriptionConfirmed));
                }
                Action::Text(TextAction::CancelEditMultiLine) if self.is_editing => {
                    self.is_editing = false;
                    return (true, Some(TaskDetailsEvent::EditCancelled));
                }
                _ => {}
            }
        }
        (self.is_editing, None)
    }
}
