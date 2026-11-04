use std::{
    fmt::Formatter,
    fs::{create_dir_all, read_to_string, write},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use log::error;
use serde::de::DeserializeOwned;

#[derive(Debug)]
#[allow(dead_code)]
pub enum TomlFileError {
    NoPath(String),
    IoError(std::io::Error),
    TomlDeError(toml::de::Error),
    TomlSeError(toml::ser::Error),
}

impl TomlFileError {
    pub fn is_file_not_found(&self) -> bool {
        matches!(self, TomlFileError::IoError(err) if err.kind() == ErrorKind::NotFound)
    }
}

impl std::fmt::Display for TomlFileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:#?}")
    }
}

impl std::error::Error for TomlFileError {}

impl From<std::io::Error> for TomlFileError {
    fn from(value: std::io::Error) -> Self {
        TomlFileError::IoError(value)
    }
}

impl From<toml::de::Error> for TomlFileError {
    fn from(value: toml::de::Error) -> Self {
        TomlFileError::TomlDeError(value)
    }
}

impl From<toml::ser::Error> for TomlFileError {
    fn from(value: toml::ser::Error) -> Self {
        TomlFileError::TomlSeError(value)
    }
}

/// `$XDG_CONFIG_HOME/vaporeon`, or `~/.config/vaporeon` if `XDG_CONFIG_HOME` isn't set.
pub fn get_config_dir_path() -> Result<PathBuf, TomlFileError> {
    xdg_dir_path(std::env::var_os("XDG_CONFIG_HOME"), home::home_dir(), ".config")
        .ok_or(TomlFileError::NoPath(".config/vaporeon".to_string()))
}

/// `$XDG_STATE_HOME/vaporeon`, or `~/.local/state/vaporeon` if `XDG_STATE_HOME` isn't set.
/// For things worth keeping between runs, but not worth backing up, like logs.
pub fn get_state_dir_path() -> Option<PathBuf> {
    xdg_dir_path(std::env::var_os("XDG_STATE_HOME"), home::home_dir(), ".local/state")
}

/// `<xdg_dir>/vaporeon`, or `<home>/<default>/vaporeon` if `xdg_dir` (the value of an `XDG_*_HOME` variable)
/// isn't set.
fn xdg_dir_path(xdg_dir: Option<std::ffi::OsString>, home: Option<PathBuf>, default: &str) -> Option<PathBuf> {
    // Per the XDG spec, relative paths are invalid and must be ignored.
    let xdg_dir = xdg_dir.map(PathBuf::from).filter(|path| path.is_absolute());
    xdg_dir
        .or_else(|| home.map(|home| home.join(default)))
        .map(|dir| dir.join("vaporeon"))
}

pub fn get_config_file_path(file_name: &str) -> Result<PathBuf, TomlFileError> {
    get_config_dir_path().map(|path| path.as_path().join(file_name).with_extension("toml"))
}

fn create_dir() -> Result<(), TomlFileError> {
    let path = get_config_dir_path()?;
    Ok(create_dir_all(path)?)
}

pub fn read_toml_file_path<T>(path: &Path) -> Result<T, TomlFileError>
where
    T: DeserializeOwned,
{
    let string = read_to_string(path)?;
    Ok(toml::from_str(&string)?)
}

pub fn read_toml_file<T>(file_name: &str) -> Result<T, TomlFileError>
where
    T: DeserializeOwned,
{
    let path = get_config_file_path(file_name)?;
    let string = read_to_string(&path)?;
    Ok(toml::from_str(&string)?)
}

pub fn read_toml_file_or_default<T>(file_name: &str) -> T
where
    T: DeserializeOwned + Default,
{
    read_toml_file(file_name).unwrap_or_else(|err| {
        error!("Error in '{file_name}' file. Will use default values. Error was: \n{err:#?}");
        T::default()
    })
}

pub fn write_toml_file<T>(file_name: &str, file_contents: &T) -> Result<(), TomlFileError>
where
    T: serde::ser::Serialize,
{
    create_dir()?;
    let path = get_config_file_path(file_name)?;
    let serialized = toml::to_string_pretty(file_contents)?;
    write(path, serialized)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_dirs() {
        let home = Some(PathBuf::from("/home/taro"));
        let config = |xdg: Option<&str>| xdg_dir_path(xdg.map(Into::into), home.clone(), ".config");
        assert_eq!(config(Some("/xdg/config")), Some(PathBuf::from("/xdg/config/vaporeon")));
        assert_eq!(config(None), Some(PathBuf::from("/home/taro/.config/vaporeon")));
        // Empty or relative: ignored.
        assert_eq!(config(Some("")), Some(PathBuf::from("/home/taro/.config/vaporeon")));
        assert_eq!(
            config(Some("relative")),
            Some(PathBuf::from("/home/taro/.config/vaporeon"))
        );

        let state = |xdg: Option<&str>| xdg_dir_path(xdg.map(Into::into), home.clone(), ".local/state");
        assert_eq!(state(None), Some(PathBuf::from("/home/taro/.local/state/vaporeon")));
        assert_eq!(state(Some("/xdg/state")), Some(PathBuf::from("/xdg/state/vaporeon")));
    }
}
