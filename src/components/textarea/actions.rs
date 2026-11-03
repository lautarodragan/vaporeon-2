use crate::{
    action::{Action, OnActions, TextAction},
    components::textarea::Textarea,
};

#[derive(Debug)]
pub enum TextAreaEvent {
    TextEditConfirmed,
}

impl OnActions<Action, (bool, Option<TextAreaEvent>)> for &mut Textarea {
    fn on_actions(self, actions: Vec<Action>) -> (bool, Option<TextAreaEvent>) {
        for action in actions {
            match action {
                Action::Text(TextAction::EnterEditMode) if !self.is_editing => {
                    self.is_editing = true;
                    return (true, None);
                }
                Action::Text(text_action) if self.is_editing => match text_action {
                    TextAction::ConfirmEditMultiLine => {
                        self.is_editing = false;
                        return (true, Some(TextAreaEvent::TextEditConfirmed));
                    }
                    TextAction::CancelEdit => {
                        self.is_editing = false;
                        return (true, None);
                    }
                    TextAction::MoveCaretToNextGrapheme => {
                        if (self.caret_position.char) < self.selected_line_len() {
                            self.caret_position.char += 1;
                        } else if self.caret_position.line + 1 < self.lines.len() {
                            self.caret_position.line += 1;
                            self.caret_position.char = 0;
                        }
                        return (true, None);
                    }
                    TextAction::MoveCaretToPreviousGrapheme => {
                        if self.caret_position.char > 0 {
                            self.caret_position.char -= 1;
                        } else if self.caret_position.line > 0 {
                            self.caret_position.line -= 1;
                            self.caret_position.char = self.selected_line_len();
                        }
                        return (true, None);
                    }
                    TextAction::MoveCaretToNextLine if self.caret_position.line + 1 < self.lines.len() => {
                        self.caret_position.line += 1;
                        self.caret_position.char = self.caret_position.char.min(self.selected_line_len());
                        return (true, None);
                    }
                    TextAction::MoveCaretToPreviousLine if self.caret_position.line > 0 => {
                        self.caret_position.line -= 1;
                        self.caret_position.char = self.caret_position.char.min(self.selected_line_len());
                        return (true, None);
                    }
                    TextAction::MoveCaretToEndOfLine => {
                        self.caret_position.char = self.selected_line_len();
                        return (true, None);
                    }
                    TextAction::MoveCaretToBeginningOfLine => {
                        self.caret_position.char = 0;
                        return (true, None);
                    }
                    TextAction::MoveCaretToEndOfText => {
                        self.caret_position.line = self.lines.len().saturating_sub(1);
                        self.caret_position.char = self.selected_line_len();
                        return (true, None);
                    }
                    TextAction::MoveCaretToBeginningOfText => {
                        self.caret_position.char = 0;
                        self.caret_position.line = 0;
                        return (true, None);
                    }
                    TextAction::NewLine => {
                        let graphemes = self.selected_line_graphemes();
                        let new_line_string = if let Some(s1) = graphemes.get(self.caret_position.char) {
                            let byte_index = s1.0;
                            let s2 = self.lines[self.caret_position.line][byte_index..].to_string();
                            self.lines[self.caret_position.line].truncate(byte_index);
                            s2
                        } else {
                            "".to_string()
                        };
                        self.lines.insert(self.caret_position.line + 1, new_line_string);

                        self.caret_position.line += 1;
                        self.caret_position.char = 0;
                        return (true, None);
                    }
                    TextAction::Char(char) => {
                        log::debug!("text char {char}");

                        let graphemes = self.selected_line_graphemes();
                        let byte_index = graphemes.get(self.caret_position.char).map(|a| a.0);

                        if let Some(byte_index) = byte_index {
                            self.lines[self.caret_position.line].insert(byte_index, char);
                        } else {
                            self.lines[self.caret_position.line] += char.to_string().as_str();
                        }

                        self.caret_position.char += 1;

                        return (true, None);
                    }
                    TextAction::DeleteBack => {
                        if self.lines.is_empty() {
                            return (true, None);
                        }

                        if self.caret_position.char > 0 {
                            let graphemes = self.selected_line_graphemes();
                            let caret_position = self.caret_position.char.min(graphemes.len()).saturating_sub(1);

                            let grapheme = graphemes[caret_position];
                            log::debug!("remove {grapheme:?}");
                            let byte_index = grapheme.0;

                            self.lines[self.caret_position.line].remove(byte_index);
                            self.caret_position.char = caret_position;
                        } else if self.caret_position.line > 0 {
                            let removed = self.lines.remove(self.caret_position.line);
                            self.caret_position.line -= 1;
                            self.caret_position.char = self.selected_line_len();
                            self.lines[self.caret_position.line] += removed.as_str();
                        }

                        return (true, None);
                    }
                    TextAction::Delete => {
                        if self.lines.is_empty() {
                            return (true, None);
                        }
                        if self.caret_position.char < self.selected_line_len() {
                            let graphemes = self.selected_line_graphemes();
                            let caret_position = self.caret_position.char.min(graphemes.len());
                            let grapheme = graphemes[caret_position];
                            let byte_index = grapheme.0;

                            self.lines[self.caret_position.line].remove(byte_index);
                            self.caret_position.char = caret_position;
                        } else if self.caret_position.line + 1 < self.lines.len() {
                            let removed = self.lines.remove(self.caret_position.line + 1);
                            self.lines[self.caret_position.line] += removed.as_str();
                        }
                    }
                    TextAction::DeleteLine => {
                        if self.lines.is_empty() {
                            return (true, None);
                        } else if self.lines.len() == 1 {
                            self.lines[0].clear();
                        } else {
                            self.lines.remove(self.caret_position.line);
                            if self.caret_position.line >= self.lines.len() && self.caret_position.line > 0 {
                                self.caret_position.line -= 1;
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        (self.is_editing, None)
    }
}
