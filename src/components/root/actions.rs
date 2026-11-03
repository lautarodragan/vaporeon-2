use super::Root;
use crate::{
    action::{Action, DayAction, NavigationAction, OnActions},
    components::{root::root::RightPanel, task_details::TaskDetailsEvent, task_list::TaskListEvent},
};

impl OnActions for &mut Root {
    fn on_actions(self, actions: Vec<Action>) {
        if self.selected_panel == 0 {
            let (handled, _event) = self.task_list_day.on_actions(actions.clone());
            if handled {
                // When task_list_day handles any action, always refresh task_details.
                //   This covers task creation, cancelled edits, etc.
                // TODO: emit "title changed" event or something?
                self.on_selection_change();
                return;
            }
        } else if self.right_panel == RightPanel::GlobalTaskList {
            let (handled, event) = self.task_list_global.on_actions(actions.clone());
            if handled {
                match event {
                    Some(TaskListEvent::SelectionChanged) => {
                        log::debug!("Selected task (global) changed");
                    }
                    _ => {
                        log::debug!("task_list_global event {:?}", event);
                    }
                }
                return;
            }
        } else if self.right_panel == RightPanel::TaskDetails {
            let (handled, event) = self.task_details.on_actions(actions.clone());

            if handled {
                match event {
                    Some(TaskDetailsEvent::DescriptionConfirmed) => {
                        let description = self.task_details.description().to_string();
                        self.selected_task_mut().set_description(description);
                    }
                    Some(TaskDetailsEvent::EditCancelled) => {
                        // Throw away the edited text, show the task's description again.
                        self.on_selection_change();
                    }
                    None => {}
                }
                return;
            }
        }

        for action in actions {
            match action {
                Action::Navigation(navigation) => match navigation {
                    NavigationAction::FocusDayOrGlobal => {
                        self.selected_panel = if self.selected_panel == 0 && self.right_panel != RightPanel::Nothing {
                            1
                        } else {
                            0
                        };
                        return;
                    }
                    NavigationAction::FocusDay if self.right_panel == RightPanel::GlobalTaskList => {
                        self.selected_panel = 0;
                        return;
                    }
                    NavigationAction::FocusGlobal if self.right_panel == RightPanel::GlobalTaskList => {
                        self.selected_panel = 1;
                        return;
                    }
                    NavigationAction::ToggleGlobalPanel => {
                        self.right_panel = if self.right_panel == RightPanel::GlobalTaskList {
                            RightPanel::Nothing
                        } else {
                            RightPanel::GlobalTaskList
                        };
                        if self.right_panel == RightPanel::Nothing {
                            self.selected_panel = 0;
                        } else {
                            self.selected_panel = 1;
                        }
                        return;
                    }
                    NavigationAction::ToggleDetailsPanel => {
                        if self.right_panel == RightPanel::TaskDetails {
                            self.right_panel = RightPanel::Nothing;
                            self.selected_panel = 0;
                        } else {
                            self.right_panel = RightPanel::TaskDetails;
                            let task = self.task_list_day.selected_status().selected_task();
                            self.task_details.set_title(task.title());
                            self.task_details.set_description(task.description());
                            self.selected_panel = 1;
                        }
                        return;
                    }
                    NavigationAction::GoToNextDay
                        if self.selected_panel == 0 || self.right_panel == RightPanel::GlobalTaskList =>
                    {
                        self.go_to_next_day();
                        return;
                    }
                    NavigationAction::GoToPreviousDay
                        if self.selected_panel == 0 || self.right_panel == RightPanel::GlobalTaskList =>
                    {
                        self.go_to_previous_day();
                        return;
                    }
                    NavigationAction::GoToToday
                        if self.selected_panel == 0 || self.right_panel == RightPanel::GlobalTaskList =>
                    {
                        self.go_to_today();
                        return;
                    }
                    _ => {}
                },
                Action::Day(day_action) => match day_action {
                    DayAction::SaveAsTemplate => {
                        self.save_as_template();
                    }
                    DayAction::LoadTemplate => {
                        self.load_from_template();
                    }
                    DayAction::FilterProjectsNone => {
                        self.set_filter(None);
                    }
                    DayAction::FilterProjects1 => {
                        self.set_filter(Some(0));
                    }
                    DayAction::FilterProjects2 => {
                        self.set_filter(Some(1));
                    }
                    DayAction::FilterProjects3 => {
                        self.set_filter(Some(2));
                    }
                    DayAction::FilterProjects4 => {
                        self.set_filter(Some(3));
                    }
                    DayAction::CopyPreviousDayTodoAndDoing => {
                        self.copy_previous_day_todo_and_doing();
                    }
                },
                _ => {}
            }
        }
    }
}
