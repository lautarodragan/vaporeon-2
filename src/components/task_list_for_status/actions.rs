use super::TaskListForStatus;
use crate::{
    action::{Action, NavigationAction, OnActions},
    components::task_list::TaskListEvent,
};

impl OnActions<Action, (bool, Option<TaskListEvent>)> for &mut TaskListForStatus {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TaskListEvent>) {
        // log::debug!("{actions:?}");

        let (handled, event) = self.tasks_by_project[self.selected_project].on_actions(actions.clone());

        if handled {
            return (true, event);
        }

        for action in actions {
            if let Action::Navigation(navigation) = action {
                match navigation {
                    NavigationAction::Up | NavigationAction::PageUp
                        if self.selected_project > 0 && self.project_filter.is_none() =>
                    {
                        self.selected_project -= 1;
                        if navigation == NavigationAction::Up {
                            self.selected_project_mut().select_last();
                        }
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::Down | NavigationAction::PageDown
                        if self.selected_project + 1 < self.tasks_by_project.len() && self.project_filter.is_none() =>
                    {
                        self.selected_project += 1;
                        if navigation == NavigationAction::Down {
                            self.selected_project_mut().selected_task = 0;
                        }
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    _ => {}
                }
            }
        }

        (false, None)
    }
}
