//! What the app needs to know to show the data: where it is, and its projects.
//!
//! It comes from two places:
//! - `settings.toml` (in the config directory): this device's settings. Where the data is (`storage_path`), and
//!   which projects to hide here (`hidden_projects`).
//! - `vaporeon.toml` (in the data repository, see [`crate::repo_config`]): the projects, which are the same on every
//!   device.
//!
//! Older versions kept the projects in `settings.toml`. If the repository has no `vaporeon.toml` yet, they're moved
//! there. `settings.toml` is written when closing, in the current form: without the projects, once they're in the
//! repository.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    models::Project,
    repo_config::{self, RepoConfig, RepoProject},
    toml_io::{TomlFileError, read_toml_file_or_default, write_toml_file},
};

#[derive(Debug, Clone)]
pub struct Settings {
    pub storage_path: PathBuf,
    /// In the order they're shown, with whether they're hidden on this device.
    pub projects: Vec<Project>,
}

/// `settings.toml`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct LocalSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_path: Option<PathBuf>,
    /// Ids of the projects not shown on this device.
    #[serde(skip_serializing_if = "Option::is_none")]
    hidden_projects: Option<Vec<usize>>,
    /// Older versions kept the projects here. Read only to move them to `vaporeon.toml`, and for which ones were
    /// hidden. Written only while they couldn't be moved.
    #[serde(skip_serializing_if = "Option::is_none")]
    projects: Option<Vec<Project>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            storage_path: storage_path_default(),
            projects: default_projects()
                .into_iter()
                .map(|p| Project::new(p.id, p.name))
                .collect(),
        }
    }
}

fn storage_path_default() -> PathBuf {
    let home = home::home_dir().unwrap_or(PathBuf::from("~"));
    home.join("vaporeon")
}

/// When there's nothing else. Never written to the repository: another device may have its real projects there,
/// not pulled yet.
fn default_projects() -> Vec<RepoProject> {
    ["Home", "Work", "Hobbies"]
        .into_iter()
        .enumerate()
        .map(|(id, name)| RepoProject {
            id,
            name: name.to_string(),
        })
        .collect()
}

impl Settings {
    pub fn load() -> Self {
        let local: LocalSettings = read_toml_file_or_default("settings");
        let storage_path = local.storage_path.clone().unwrap_or_else(storage_path_default);
        let repo = RepoConfig::load(&storage_path);
        let (settings, to_write) = resolve(local, storage_path, repo);
        if let Some(config) = to_write {
            match config.save(&settings.storage_path) {
                Ok(()) => log::info!(
                    "Moved the projects from settings.toml to {} in {:?}. They're now shared by every device.",
                    repo_config::FILE_NAME,
                    settings.storage_path
                ),
                Err(err) => log::error!("Couldn't write {}: {err}", repo_config::FILE_NAME),
            }
        }
        settings
    }

    /// Writes `settings.toml`. The projects only if the repository doesn't have them (e.g. writing `vaporeon.toml`
    /// failed), so they're not lost.
    pub fn save(&self) {
        let in_repository = RepoConfig::path(&self.storage_path).exists();
        let local = LocalSettings {
            storage_path: Some(self.storage_path.clone()),
            hidden_projects: Some(self.projects.iter().filter(|p| p.hidden).map(|p| p.id).collect()),
            projects: (!in_repository).then(|| self.projects.clone()),
        };
        if let Err(err) = write_toml_file("settings", &local) {
            log::error!("Error while saving settings: {err:?}");
        }
    }

    /// The projects in the repository's `vaporeon.toml`, if they changed (e.g. pulled from the git remote).
    pub fn with_reloaded_projects(&self) -> Option<Settings> {
        let config = match RepoConfig::load(&self.storage_path) {
            Ok(Some(config)) => config,
            Ok(None) => return None,
            Err(err) => {
                log::error!("Error in {}: {err}", repo_config::FILE_NAME);
                return None;
            }
        };
        let hidden: Vec<usize> = self.projects.iter().filter(|p| p.hidden).map(|p| p.id).collect();
        let projects = projects(&config.projects, &hidden);
        let changed = projects.len() != self.projects.len()
            || projects
                .iter()
                .zip(&self.projects)
                .any(|(a, b)| a.id != b.id || a.name != b.name || a.hidden != b.hidden);
        changed.then(|| Settings {
            storage_path: self.storage_path.clone(),
            projects,
        })
    }

    pub fn visible_projects(&self) -> Vec<Project> {
        self.projects.iter().filter(|p| !p.hidden).cloned().collect()
    }
}

/// The settings, from `settings.toml` and the repository's `vaporeon.toml`. Also the `vaporeon.toml` to write, if the
/// projects have to be moved there.
fn resolve(
    local: LocalSettings,
    storage_path: PathBuf,
    repo: Result<Option<RepoConfig>, TomlFileError>,
) -> (Settings, Option<RepoConfig>) {
    let legacy = local.projects.map(|projects| {
        let config = RepoConfig::new(
            projects
                .iter()
                .map(|p| RepoProject {
                    id: p.id,
                    name: p.name.clone(),
                })
                .collect(),
        );
        let hidden: Vec<usize> = projects.iter().filter(|p| p.hidden).map(|p| p.id).collect();
        (config, hidden)
    });
    // Older versions said which projects were hidden in the projects themselves.
    let hidden = local
        .hidden_projects
        .or_else(|| legacy.as_ref().map(|(_, hidden)| hidden.clone()))
        .unwrap_or_default();

    let (repo_projects, to_write) = match repo {
        Ok(Some(config)) => {
            if config.version > repo_config::VERSION {
                log::warn!(
                    "{} is version {}, newer than this app (version {}). It may not be read correctly.",
                    repo_config::FILE_NAME,
                    config.version,
                    repo_config::VERSION
                );
            }
            if legacy.is_some() {
                log::warn!(
                    "The projects in settings.toml are ignored: they're in {} now.",
                    repo_config::FILE_NAME
                );
            }
            (config.projects, None)
        }
        Ok(None) => match legacy {
            Some((config, _)) => (config.projects.clone(), Some(config)),
            None => (default_projects(), None),
        },
        // Don't overwrite a file that's there but can't be read: it may be fixed by hand.
        Err(err) => {
            log::error!("Error in {}: {err}", repo_config::FILE_NAME);
            match legacy {
                Some((config, _)) => (config.projects, None),
                None => (default_projects(), None),
            }
        }
    };

    let settings = Settings {
        storage_path,
        projects: projects(&repo_projects, &hidden),
    };
    (settings, to_write)
}

fn projects(repo_projects: &[RepoProject], hidden: &[usize]) -> Vec<Project> {
    repo_projects
        .iter()
        .map(|p| Project {
            id: p.id,
            name: p.name.clone(),
            hidden: hidden.contains(&p.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(text: &str) -> LocalSettings {
        toml::from_str(text).unwrap()
    }

    fn names(settings: &Settings) -> Vec<(usize, &str, bool)> {
        settings
            .projects
            .iter()
            .map(|p| (p.id, p.name.as_str(), p.hidden))
            .collect()
    }

    const LEGACY: &str = r#"
        storage_path = "/data"
        [[projects]]
        id = 0
        name = "Casita"
        [[projects]]
        id = 2
        name = "RedBee"
        hidden = true
    "#;

    #[test]
    fn moves_the_projects_to_the_repository_if_they_arent_there() {
        let (settings, to_write) = resolve(local(LEGACY), "/data".into(), Ok(None));
        assert_eq!(names(&settings), [(0, "Casita", false), (2, "RedBee", true)]);
        let config = to_write.unwrap();
        assert_eq!(config.version, repo_config::VERSION);
        assert_eq!(config.projects.len(), 2);
        assert_eq!(config.projects[1].name, "RedBee");
    }

    #[test]
    fn the_repository_has_the_projects_and_the_device_which_are_hidden() {
        let repo = RepoConfig::new(vec![
            RepoProject {
                id: 2,
                name: "Work".to_string(),
            },
            RepoProject {
                id: 0,
                name: "Home".to_string(),
            },
        ]);
        let (settings, to_write) = resolve(local("hidden_projects = [0]\n"), "/data".into(), Ok(Some(repo.clone())));
        assert_eq!(names(&settings), [(2, "Work", false), (0, "Home", true)]);
        assert_eq!(to_write, None);

        // Projects still in settings.toml are ignored, except for which ones were hidden.
        let (settings, to_write) = resolve(local(LEGACY), "/data".into(), Ok(Some(repo)));
        assert_eq!(names(&settings), [(2, "Work", true), (0, "Home", false)]);
        assert_eq!(to_write, None);
    }

    #[test]
    fn defaults_are_never_written_to_the_repository() {
        let (settings, to_write) = resolve(local(""), "/data".into(), Ok(None));
        assert_eq!(names(&settings), [
            (0, "Home", false),
            (1, "Work", false),
            (2, "Hobbies", false)
        ]);
        assert_eq!(to_write, None);
    }

    #[test]
    fn an_unreadable_repository_config_isnt_overwritten() {
        let error = toml::from_str::<RepoConfig>("version = ").unwrap_err();
        let (settings, to_write) = resolve(local(LEGACY), "/data".into(), Err(error.into()));
        assert_eq!(names(&settings), [(0, "Casita", false), (2, "RedBee", true)]);
        assert_eq!(to_write, None);
    }
}
