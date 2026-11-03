use super::editable_task::EditableTask;
use crate::action::{Action, NavigationAction, OnActions, TextAction};

impl OnActions<Action, bool> for &mut EditableTask {
    fn on_actions(self, actions: Vec<Action>) -> bool {
        // log::debug!("{actions:?}");

        for action in actions {
            match action {
                Action::Navigation(navigation) if self.is_editing => match navigation {
                    NavigationAction::Left => {
                        if self.caret_position > 0 {
                            self.caret_position -= 1;
                            return true;
                        }
                    }
                    NavigationAction::Right => {
                        let graphemes = self.graphemes();
                        if self.caret_position < graphemes.len() {
                            self.caret_position += 1;
                            return true;
                        }
                    }
                    NavigationAction::Home => {
                        self.caret_position = 0;
                        return true;
                    }
                    NavigationAction::End => {
                        let graphemes = self.graphemes();
                        self.caret_position = graphemes.len();
                        return true;
                    }
                    _ => {}
                },
                Action::Text(TextAction::EnterEditMode) if !self.is_editing => {
                    self.is_editing = true;
                    return true;
                }
                Action::Text(text) if self.is_editing => {
                    match text {
                        TextAction::ConfirmEdit => {
                            self.is_editing = false;
                            return true;
                        }
                        TextAction::CancelEdit => {
                            self.is_editing = false;
                            return true;
                        }
                        TextAction::Char(char) => {
                            let graphemes = self.graphemes();
                            if self.caret_position >= graphemes.len() {
                                self.title.push(char);
                            } else {
                                log::debug!(
                                    "char({char}, {}, {:?})",
                                    self.caret_position,
                                    graphemes[self.caret_position]
                                );
                                self.title.insert(graphemes[self.caret_position].0, char);
                            }
                            self.caret_position += 1;
                            return true;
                        }
                        TextAction::Delete => {
                            if self.title.is_empty() {
                                // if self.tasks.len() > 1 && self.selected_task + 1 < self.tasks.len() {
                                //     self.tasks.remove(self.selected_task);
                                //     if self.selected_task >= self.tasks.len() {
                                //         self.selected_task -= 1;
                                //     }
                                //     self.caret_position = 0;
                                // }
                                return true;
                            } else {
                                let graphemes = self.graphemes();
                                let caret_position = self.caret_position.min(graphemes.len());
                                if caret_position < graphemes.len() {
                                    self.title.remove(graphemes[caret_position].0);
                                }
                                self.caret_position = caret_position;
                            }
                            return true;
                        }
                        TextAction::DeleteBack => {
                            if self.title.is_empty() {
                                // if self.tasks.len() > 1 && self.selected_task > 0 {
                                //     self.tasks.remove(self.selected_task);
                                //     self.selected_task -= 1;
                                //     self.caret_position = self.text.len()
                                // }
                                return true;
                            }

                            if self.caret_position > 0 {
                                let graphemes = self.graphemes();
                                let caret_position = self.caret_position.min(graphemes.len()).saturating_sub(1);

                                let grapheme = graphemes[caret_position];
                                log::debug!("remove {grapheme:?}");

                                self.title.remove(grapheme.0);

                                self.caret_position = caret_position;
                            }
                            return true;
                        }
                        TextAction::DeleteBackWord => {
                            if self.caret_position == 0 {
                                return true;
                            }

                            let graphemes = self.graphemes();

                            let caret_is_on_space = graphemes.get(self.caret_position - 1).is_none_or(|g| g.1 == " ");

                            let start_grapheme_index = (0..self.caret_position - 1)
                                .rev()
                                .find(|i| caret_is_on_space != (graphemes[*i].1 == " "))
                                .map(|i| i + 1)
                                .unwrap_or(0);

                            let end_byte = graphemes
                                .get(self.caret_position)
                                .map(|(i, _)| *i)
                                .unwrap_or(self.title.len());

                            self.title.drain(graphemes[start_grapheme_index].0..end_byte);
                            self.caret_position = start_grapheme_index;
                            return true;
                        }
                        TextAction::MoveCaretToPreviousWord => {
                            if self.caret_position == 0 {
                                return true;
                            }

                            let graphemes = self.graphemes();

                            let caret_is_on_space = graphemes.get(self.caret_position - 1).is_none_or(|g| g.1 == " ");

                            let start_grapheme_index = (0..self.caret_position - 1)
                                .rev()
                                .find(|i| caret_is_on_space != (graphemes[*i].1 == " "))
                                .map(|i| i + 1)
                                .unwrap_or(0);

                            self.caret_position = start_grapheme_index;
                            return true;
                        }
                        TextAction::MoveCaretToNextWord => {
                            let graphemes = self.graphemes();

                            if self.caret_position == graphemes.len() {
                                return true;
                            }

                            let caret_is_on_space = graphemes.get(self.caret_position).is_none_or(|g| g.1 == " ");

                            let start_grapheme_index = (self.caret_position..graphemes.len())
                                .find(|i| caret_is_on_space != (graphemes[*i].1 == " "))
                                .unwrap_or(graphemes.len());

                            self.caret_position = start_grapheme_index;
                            return true;
                        }
                        _ => {}
                    }
                }
                _ => {}
            };
        }

        false
    }
}
