use crate::{
    action::{Action, ApplicationAction, NavigationAction, OnActions, TaskAction},
    clipboard,
    components::{EditableTask, task_list::TaskListEvent, task_list_for_project::TaskListForProject},
};

impl OnActions<Action, (bool, Option<TaskListEvent>)> for &mut TaskListForProject {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TaskListEvent>) {
        // log::debug!("{actions:?}");

        if self.tasks[self.selected_task].on_actions(actions.clone()) {
            return (true, None);
        }

        if self.is_editing() {
            return (true, None);
        }

        for action in actions {
            match action {
                Action::Application(ApplicationAction::PasteText(text)) => {
                    // self.selected_task += 1;
                    // TODO: [task_title, ...task_details] = pasted_text.split("\n")
                    self.tasks
                        .insert(self.selected_task, EditableTask::new(text, "".to_string()));
                    return (true, None);
                }
                Action::Navigation(navigation) => match navigation {
                    NavigationAction::Up if self.selected_task > 0 => {
                        self.selected_task -= 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::Down if self.selected_task + 1 < self.tasks.len() => {
                        self.selected_task += 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::SelectFirstTaskInProject => {
                        self.selected_task = 0;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::SelectLastTaskInProject => {
                        self.selected_task = self.tasks.len().saturating_sub(1);
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    _ => {}
                },
                Action::Task(task) => match task {
                    TaskAction::Create => {
                        let mut task = EditableTask::empty();
                        task.set_is_editing(true);
                        self.tasks.insert(self.selected_task + 1, task);
                        self.selected_task += 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    TaskAction::Duplicate if !self.tasks.is_empty() => {
                        let task = self.tasks[self.selected_task].clone();
                        self.tasks.insert(self.selected_task + 1, task);
                        self.selected_task += 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    TaskAction::Copy if !self.tasks.is_empty() => {
                        let task = &self.tasks[self.selected_task];
                        clipboard::copy(task.as_str());
                    }
                    TaskAction::Cut if self.tasks.len() > 1 => {
                        let task = self.tasks.remove(self.selected_task);
                        clipboard::copy(task.as_str());

                        if self.selected_task >= self.tasks.len() {
                            self.selected_task -= 1;
                            return (true, Some(TaskListEvent::SelectionChanged));
                        }
                    }
                    TaskAction::Cut if self.tasks.len() == 1 => {
                        let task = self.tasks[0].take_title();
                        clipboard::copy(task.as_str());

                        // self.tasks[0].clear();
                        return (true, None);
                    }
                    TaskAction::MoveBackwardInsideProject if self.selected_task > 0 => {
                        self.tasks.swap(self.selected_task, self.selected_task - 1);
                        self.selected_task -= 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    TaskAction::MoveForwardInsideProject if self.selected_task + 1 < self.tasks.len() => {
                        self.tasks.swap(self.selected_task, self.selected_task + 1);
                        self.selected_task += 1;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    TaskAction::Delete if self.tasks.len() > 1 => {
                        self.tasks.remove(self.selected_task);
                        if self.selected_task >= self.tasks.len() {
                            self.selected_task -= 1;
                            return (true, Some(TaskListEvent::SelectionChanged));
                        }
                    }
                    TaskAction::Delete if self.tasks.len() == 1 => {
                        self.tasks[0].clear();
                        return (true, None);
                    }
                    _ => {}
                },
                _ => {}
            };
        }

        (false, None)
    }
}
