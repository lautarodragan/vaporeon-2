use unicode_segmentation::UnicodeSegmentation;

use crate::models::Task as DayTask;

#[derive(Clone, Debug)]
pub struct EditableTask {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) is_editing: bool,

    /// Position of the caret. Measured in graphemes, **not** bytes.
    ///
    /// For example, the string "aéi" is 3 graphemes, but 4 bytes long. The graphemes are at bytes 0, 1, and 3.
    ///
    /// Valid values are `[zero, text.len()]` (both ends **included**). This allows placing
    /// the caret beyond the last byte of `text`, which means `text[caret_position]` **can panic**.
    pub(super) caret_position: usize,
}

impl EditableTask {
    pub fn new(text: String, details: String) -> Self {
        let graphemes = UnicodeSegmentation::grapheme_indices(text.as_str(), true).collect::<Vec<(usize, &str)>>();
        Self {
            is_editing: false,
            caret_position: graphemes.len(),
            title: text,
            description: details,
        }
    }

    pub fn empty() -> Self {
        Self::new("".to_string(), "".to_string())
    }

    pub fn from_day_task(day_task: &DayTask) -> Self {
        Self::new(day_task.text.clone(), day_task.details.clone().unwrap_or_default())
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    pub fn set_is_editing(&mut self, v: bool) {
        self.is_editing = v;
    }

    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty()
    }

    pub fn as_str(&self) -> &str {
        self.title.as_str()
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }

    pub fn take_title(&mut self) -> String {
        self.caret_position = 0;
        std::mem::take(&mut self.title)
    }

    pub fn set_title(&mut self, v: String) {
        self.caret_position = 0;
        self.title = v;
    }

    pub fn into_title(self) -> String {
        self.title
    }

    pub fn take_description(&mut self) -> String {
        std::mem::take(&mut self.description)
    }

    pub fn into_parts(self) -> (String, String) {
        (self.title, self.description)
    }

    pub fn clear(&mut self) {
        self.caret_position = 0;
        self.title.clear()
    }

    pub fn graphemes(&self) -> Vec<(usize, &str)> {
        UnicodeSegmentation::grapheme_indices(self.title.as_str(), true).collect::<Vec<(usize, &str)>>()
    }

    /// Byte index (inside `title`) of the caret.
    pub fn caret_byte_index(&self) -> usize {
        self.graphemes()
            .get(self.caret_position)
            .map(|(i, _)| *i)
            .unwrap_or(self.title.len())
    }
}
