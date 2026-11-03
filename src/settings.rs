use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    models::Project,
    toml_io::{read_toml_file_or_default, write_toml_file},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    #[serde(default = "storage_path_default")]
    pub storage_path: PathBuf,
    pub projects: Vec<Project>,
}

impl Default for Settings {
    fn default() -> Self {
        let storage_path = storage_path_default();
        Self {
            storage_path,
            projects: vec![
                Project::new(0, "Home".to_string()),
                Project::new(1, "Work".to_string()),
                Project::new(2, "Hobbies".to_string()),
            ],
        }
    }
}

fn storage_path_default() -> PathBuf {
    let home = home::home_dir().unwrap_or(PathBuf::from("~"));
    home.join("vaporeon")
}

impl Settings {
    pub fn load() -> Self {
        read_toml_file_or_default("settings")
    }

    pub fn save(&self) {
        if let Err(err) = write_toml_file("settings", &self) {
            log::error!("Error while saving settings: {err:?}");
        }
    }

    pub fn visible_projects(&self) -> Vec<Project> {
        self.projects.iter().filter(|p| !p.hidden).cloned().collect()
    }
}
