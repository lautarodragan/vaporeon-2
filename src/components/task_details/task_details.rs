pub struct TaskDetails {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) is_editing: bool,
}

impl TaskDetails {
    pub fn new() -> Self {
        Self {
            title: "".to_string(),
            description: "".to_string(),
            is_editing: false,
        }
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    pub fn set_is_editing(&mut self, is_editing: bool) {
        self.is_editing = is_editing;
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn description(&self) -> &str {
        self.description.as_str()
    }

    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }
}
