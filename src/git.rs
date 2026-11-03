use std::path::Path;

use chrono::Utc;

#[derive(Debug)]
#[allow(unused)]
pub enum GitError {
    IoError(std::io::Error),
    FromUtf8Error(std::string::FromUtf8Error),
    UnexpectedOutput(String),
    GitExitStatusNonZero,
}

impl From<std::io::Error> for GitError {
    fn from(value: std::io::Error) -> Self {
        GitError::IoError(value)
    }
}

impl From<std::string::FromUtf8Error> for GitError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        GitError::FromUtf8Error(value)
    }
}

pub fn run_git<I, S>(path: &Path, args: I) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stdout = String::from_utf8(output.stdout);
        let stderr = String::from_utf8(output.stderr);
        log::warn!("git error stdout: {stdout:?}");
        log::warn!("git error stderr: {stderr:?}");
        return Err(GitError::GitExitStatusNonZero);
    }

    let stdout = String::from_utf8(output.stdout)?;

    Ok(stdout)
}

pub fn date_of_most_recent_commit(path: &Path) -> Result<String, GitError> {
    const DATE: &str = "Date:";

    let s = run_git(path, ["log", "-n", "1", "--date=iso-strict"])?;
    let line = s.lines().map(|line| line.trim()).find(|line| line.starts_with("Date:"));

    let Some(line) = line else {
        return Err(GitError::UnexpectedOutput(s));
    };

    let date = line[DATE.len()..].trim().to_string();

    Ok(date)
}

pub fn add(path: &Path) -> Result<(), GitError> {
    run_git(path, ["add", "*.toml"]).map(|_| ())
}

pub fn commit(path: &Path) -> Result<(), GitError> {
    let date = Utc::now().to_string();
    let message = "automated vaporeon commit at ".to_string() + date.as_str();
    let output = run_git(path, [
        "commit",
        "--author",
        "Vaporeon <vaporeon@taro.codes>",
        "-m",
        message.as_str(),
    ])?;
    log::debug!("git commit output: \n{output}");
    Ok(())
}
