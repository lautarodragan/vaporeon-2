use crate::{
    action::{Action, OnActions},
    components::{task_details::TaskDetails, textarea::TextAreaEvent},
};

impl OnActions<Action, (bool, Option<TextAreaEvent>)> for &mut TaskDetails {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TextAreaEvent>) {
        self.description.on_actions(actions)
    }
}
