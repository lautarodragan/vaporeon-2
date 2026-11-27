//! The configuration of the data repository: `vaporeon.toml`, next to the data files. What every device that uses the
//! data needs to know about it, like which project each `project_id` is. See `docs/file-format.md`.
//!
//! What's up to each device (where the data is, which projects to hide) is in `settings.toml` instead
//! (see [`crate::settings`]).

use std::{
    fmt::Write,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{format::escape_single_line, toml_io::TomlFileError};

pub const FILE_NAME: &str = "vaporeon.toml";
/// The version of the format this app reads and writes.
pub const VERSION: u32 = 1;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RepoConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    /// In the order they're shown.
    #[serde(default)]
    pub projects: Vec<RepoProject>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RepoProject {
    pub id: usize,
    pub name: String,
}

fn default_version() -> u32 {
    VERSION
}

impl RepoConfig {
    pub fn new(projects: Vec<RepoProject>) -> Self {
        Self {
            version: VERSION,
            projects,
        }
    }

    pub fn path(storage_path: &Path) -> PathBuf {
        storage_path.join(FILE_NAME)
    }

    /// None if there's no such file.
    pub fn load(storage_path: &Path) -> Result<Option<Self>, TomlFileError> {
        match std::fs::read_to_string(Self::path(storage_path)) {
            Ok(text) => Ok(Some(toml::from_str(&text)?)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn save(&self, storage_path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(storage_path)?;
        std::fs::write(Self::path(storage_path), self.to_canonical_string())
    }

    /// The file's contents, in its one canonical form (like the data files, see `format.rs`).
    pub fn to_canonical_string(&self) -> String {
        let mut out = String::new();
        writeln!(out, "version = {}", self.version).unwrap();
        for project in &self.projects {
            out.push_str("\n[[projects]]\n");
            writeln!(out, "id = {}", project.id).unwrap();
            writeln!(out, "name = \"{}\"", escape_single_line(&project.name)).unwrap();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: usize, name: &str) -> RepoProject {
        RepoProject {
            id,
            name: name.to_string(),
        }
    }

    #[test]
    fn writes_the_canonical_form_and_reads_it_back() {
        let config = RepoConfig::new(vec![project(2, "RedBee"), project(0, "Casita \"home\"")]);
        let text = config.to_canonical_string();
        assert_eq!(
            text,
            r#"version = 1

[[projects]]
id = 2
name = "RedBee"

[[projects]]
id = 0
name = "Casita \"home\""
"#
        );
        assert_eq!(toml::from_str::<RepoConfig>(&text).unwrap(), config);
    }

    #[test]
    fn a_missing_file_is_none() {
        let dir = std::env::temp_dir().join(format!("vaporeon-test-repo-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(RepoConfig::load(&dir).unwrap(), None);

        let config = RepoConfig::new(vec![project(0, "a")]);
        config.save(&dir).unwrap();
        assert_eq!(RepoConfig::load(&dir).unwrap(), Some(config));
        let _ = std::fs::remove_dir_all(dir);
    }
}
