use crate::components::textarea::Textarea;

pub struct TaskDetails {
    pub(super) title: String,
    pub(super) description: Textarea,
}

impl TaskDetails {
    pub fn new() -> Self {
        Self {
            title: "".to_string(),
            description: Textarea::new(),
        }
    }

    pub fn is_editing(&self) -> bool {
        self.description.is_editing()
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn description(&self) -> &Vec<String> {
        self.description.text()
    }

    pub fn set_description(&mut self, description: &str) {
        self.description.set_text(description);
    }
}
