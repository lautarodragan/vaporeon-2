use std::{
    fs::{create_dir_all, write},
    path::Path,
};

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::toml_io::{TomlFileError, read_toml_file_path};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Project {
    pub id: usize,
    pub name: String,
    #[serde(default)]
    pub hidden: bool,
}

impl Project {
    pub fn new(id: usize, name: String) -> Self {
        Self {
            id,
            name,
            hidden: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Day {
    pub date_time: NaiveDate,
    pub tasks: Vec<Task>,
}

impl Default for Day {
    fn default() -> Self {
        Self {
            date_time: Local::now().date_naive(),
            tasks: vec![],
        }
    }
}

impl Day {
    pub fn from_file(path: &Path) -> Result<Self, TomlFileError> {
        read_toml_file_path(path)
    }

    pub fn from_file_or_default(path: &Path) -> Self {
        Self::from_file(path).unwrap_or_else(|err| {
            if !err.is_file_not_found() {
                log::error!("Error loading Day: {err:?}");
            }

            Self::default()
        })
    }

    pub fn to_file(&self, directory: &Path, file_name: String) -> Result<(), TomlFileError> {
        create_dir_all(directory)?;
        let path = directory.join(file_name + ".toml");
        let serialized = toml::to_string(&self)?;
        write(path, serialized)?;
        Ok(())
    }

    pub fn example() -> Self {
        Day {
            date_time: Local::now().date_naive(),
            tasks: vec![
                Task {
                    text: "do the dishes".to_string(),
                    project_id: 0,
                    status: TaskStatus::ToDo,
                    details: None,
                },
                Task {
                    text: "pay taxes".to_string(),
                    project_id: 0,
                    status: TaskStatus::Done,
                    details: None,
                },
                Task {
                    text: "meditate".to_string(),
                    project_id: 0,
                    status: TaskStatus::ToDo,
                    details: None,
                },
                Task {
                    text: "write tests".to_string(),
                    project_id: 1,
                    status: TaskStatus::ToDo,
                    details: None,
                },
                Task {
                    text: "implement that new feature".to_string(),
                    project_id: 1,
                    status: TaskStatus::Doing,
                    details: None,
                },
                Task {
                    text: "add another synth solo".to_string(),
                    project_id: 2,
                    status: TaskStatus::ToDo,
                    details: None,
                },
                Task {
                    text: "add more cowbell to the song".to_string(),
                    project_id: 2,
                    status: TaskStatus::Done,
                    details: None,
                },
            ],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Task {
    pub text: String,
    pub project_id: usize,
    pub status: TaskStatus,
    pub details: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    ToDo,
    Doing,
    Done,
    Dropped,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::ToDo => "TODO",
            TaskStatus::Doing => "DOING",
            TaskStatus::Done => "DONE",
            TaskStatus::Dropped => "DROPPED",
        }
    }

    pub fn all() -> Vec<TaskStatus> {
        vec![
            TaskStatus::ToDo,
            TaskStatus::Doing,
            TaskStatus::Done,
            TaskStatus::Dropped,
        ]
    }
}
