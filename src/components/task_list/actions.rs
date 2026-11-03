use super::TaskList;
use crate::{
    action::{Action, NavigationAction, OnActions, TaskAction},
    components::{EditableTask, task_list_for_project::TaskListForProject},
};

#[derive(Debug)]
pub enum TaskListEvent {
    SelectionChanged,
}

impl OnActions<Action, (bool, Option<TaskListEvent>)> for &mut TaskList {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TaskListEvent>) {
        // log::debug!("{actions:?}");

        let (handled, event) = self.status_lists[self.selected_status_list].on_actions(actions.clone());

        if handled {
            return (true, event);
        }

        for action in actions {
            match action {
                Action::Navigation(navigation) => match navigation {
                    NavigationAction::Up | NavigationAction::PageUp if self.selected_status_list > 0 => {
                        self.selected_status_list -= 1;
                        if navigation == NavigationAction::Up {
                            let list = self.selected_status_mut();
                            list.select_last();
                            list.selected_project_mut().select_last();
                        }
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::Down | NavigationAction::PageDown
                        if self.selected_status_list + 1 < self.status_lists.len() =>
                    {
                        self.selected_status_list += 1;
                        if navigation == NavigationAction::Down {
                            let list = self.selected_status_mut();
                            list.select_first();
                            list.selected_project_mut().selected_task = 0;
                        }
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::SelectFirstTaskInDay => {
                        self.selected_status_list = 0;
                        let Some(list) = self.status_lists.get_mut(self.selected_status_list) else {
                            return (true, None);
                        };
                        if self.project_filter.is_none() {
                            list.selected_project = 0;
                        }
                        let Some(list) = list.tasks_by_project.get_mut(list.selected_project) else {
                            return (true, None);
                        };
                        list.selected_task = 0;
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    NavigationAction::SelectLastTaskInDay => {
                        self.selected_status_list = self.status_lists.len().saturating_sub(1);
                        let Some(list) = self.status_lists.get_mut(self.selected_status_list) else {
                            return (true, None);
                        };
                        if self.project_filter.is_none() {
                            list.selected_project = list.tasks_by_project.len().saturating_sub(1);
                        }
                        let Some(list) = list.tasks_by_project.get_mut(list.selected_project) else {
                            return (true, None);
                        };
                        list.selected_task = list.tasks.len().saturating_sub(1);
                        return (true, Some(TaskListEvent::SelectionChanged));
                    }
                    _ => {}
                },
                Action::Task(task_action) => match task_action {
                    TaskAction::MoveToNextStatus | TaskAction::MoveToPreviousStatus => {
                        if (task_action == TaskAction::MoveToPreviousStatus && self.selected_status_list == 0)
                            || (task_action == TaskAction::MoveToNextStatus
                                && self.selected_status_list + 1 >= self.status_lists.len())
                        {
                            return (true, None);
                        }
                        let (project_id, task_text, task_details) = {
                            let list = &mut self.status_lists[self.selected_status_list];
                            let project = &mut list.tasks_by_project[list.selected_project];

                            if project.tasks[project.selected_task].is_empty() {
                                return (true, None);
                            }

                            let (task_text, task_details) = if project.tasks.len() == 1 {
                                (project.tasks[0].take_title(), project.tasks[0].take_description())
                            } else {
                                let task = project.tasks.remove(project.selected_task);

                                if project.selected_task > 0 {
                                    project.selected_task -= 1;
                                }

                                task.into_parts()
                            };
                            let project_id = project.project_id;

                            (project_id, task_text, task_details)
                        };

                        if task_action == TaskAction::MoveToNextStatus {
                            self.selected_status_list += 1;
                        } else {
                            self.selected_status_list -= 1;
                        }

                        let target_list = &mut self.status_lists[self.selected_status_list];

                        if let Some((index, target_project)) = target_list
                            .tasks_by_project
                            .iter_mut()
                            .enumerate()
                            .find(|(_, p)| p.project_id == project_id)
                        {
                            if let Some(last) = target_project.tasks.last_mut()
                                && last.is_empty()
                            {
                                last.set_title(task_text);
                                last.set_description(task_details);
                            } else {
                                target_project.tasks.push(EditableTask::new(task_text, task_details));
                            }
                            target_list.selected_project = index;
                        } else {
                            target_list.tasks_by_project.push(TaskListForProject {
                                project_id,
                                project_name: self
                                    .settings
                                    .projects
                                    .iter()
                                    .find(|p| p.id == project_id)
                                    .unwrap()
                                    .name
                                    .clone(),
                                tasks: vec![EditableTask::new(task_text, task_details)],
                                selected_task: 0,
                                show_title: false,
                            });
                            target_list.selected_project = target_list.tasks_by_project.len() - 1;
                        };
                        target_list.tasks_by_project[target_list.selected_project].selected_task =
                            target_list.tasks_by_project[target_list.selected_project].tasks.len() - 1;

                        return (true, None);
                    }
                    _ => {
                        return (false, None);
                    }
                },
                _ => {
                    return (false, None);
                }
            }
        }

        (false, None)
    }
}
