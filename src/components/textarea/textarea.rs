use unicode_segmentation::UnicodeSegmentation;

pub struct Textarea {
    pub lines: Vec<String>,
    pub is_editing: bool,
    pub caret_position: CaretPosition,
}

#[derive(Default, Debug)]
pub struct CaretPosition {
    pub line: usize,
    pub char: usize,
}

impl Textarea {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            is_editing: false,
            caret_position: CaretPosition::default(),
        }
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    /// Byte index (inside the selected line) of the caret.
    pub fn caret_byte_index(&self) -> usize {
        self.selected_line_graphemes()
            .get(self.caret_position.char)
            .map(|(i, _)| *i)
            .unwrap_or(self.selected_line().len())
    }

    pub fn selected_line(&self) -> &str {
        self.lines[self.caret_position.line].as_str()
    }

    pub fn selected_line_graphemes(&self) -> Vec<(usize, &str)> {
        UnicodeSegmentation::grapheme_indices(self.selected_line(), true).collect()
    }

    pub fn selected_line_len(&self) -> usize {
        let graphemes = self.selected_line_graphemes();
        graphemes.len()
    }

    pub fn text(&self) -> &Vec<String> {
        &self.lines
    }

    pub fn set_text(&mut self, text: &str) {
        let lines: Vec<String> = text.split('\n').map(|line| line.to_string()).collect();
        self.lines = lines;
        self.caret_position = CaretPosition::default();
    }
}
