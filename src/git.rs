use std::{path::Path, process::Stdio};

use chrono::Local;

#[derive(Debug)]
#[allow(unused)]
pub enum GitError {
    IoError(std::io::Error),
    FromUtf8Error(std::string::FromUtf8Error),
    /// What git printed to stderr.
    GitExitStatusNonZero(String),
}

impl GitError {
    /// A short description, for the user: the first meaningful line git printed.
    pub fn message(&self) -> String {
        match self {
            GitError::GitExitStatusNonZero(stderr) => stderr
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with("hint:"))
                .map(|line| {
                    line.trim_start_matches("fatal: ")
                        .trim_start_matches("error: ")
                        .to_string()
                })
                .unwrap_or_else(|| "git failed".to_string()),
            other => format!("{other:?}"),
        }
    }
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
    let mut command = std::process::Command::new("git");
    command
        .args(args)
        .current_dir(path)
        // Never wait for the user to type something (e.g. a password): there's nobody there.
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        // Give up on slow or unreachable remotes instead of hanging.
        .env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
        .env("GIT_HTTP_LOW_SPEED_TIME", "15");
    if !uses_custom_ssh_command(path) {
        command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=10");
    }
    let output = command.output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        log::debug!("git error stdout: {stdout:?}");
        log::debug!("git error stderr: {stderr:?}");
        return Err(GitError::GitExitStatusNonZero(stderr));
    }

    let stdout = String::from_utf8(output.stdout)?;

    Ok(stdout)
}

/// Whether the user set up their own ssh command for git, which we shouldn't override.
fn uses_custom_ssh_command(path: &Path) -> bool {
    if std::env::var_os("GIT_SSH_COMMAND").is_some() || std::env::var_os("GIT_SSH").is_some() {
        return true;
    }
    std::process::Command::new("git")
        .args(["config", "--get", "core.sshCommand"])
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
}

pub fn add(path: &Path) -> Result<(), GitError> {
    run_git(path, ["add", "*.toml"]).map(|_| ())
}

pub fn commit(path: &Path) -> Result<(), GitError> {
    let date = Local::now().to_rfc3339();
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

/// Commits the changes to the `*.toml` files, if there are any. Returns whether something was committed.
pub fn commit_all(path: &Path) -> Result<bool, GitError> {
    add(path)?;
    let staged = run_git(path, ["diff", "--cached", "--name-only"])?;
    if staged.trim().is_empty() {
        return Ok(false);
    }
    commit(path)?;
    Ok(true)
}

/// Whether `path` is a git repository with at least one remote.
pub fn has_remote(path: &Path) -> bool {
    run_git(path, ["remote"]).is_ok_and(|remotes| !remotes.trim().is_empty())
}

/// The remote branch the current branch tracks, if any (e.g. "origin/main").
fn upstream(path: &Path) -> Option<String> {
    run_git(path, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// "origin" if there's such a remote, otherwise the first one.
fn default_remote(path: &Path) -> Option<String> {
    let remotes = run_git(path, ["remote"]).ok()?;
    let remotes: Vec<&str> = remotes.lines().map(str::trim).filter(|r| !r.is_empty()).collect();
    remotes
        .iter()
        .find(|r| **r == "origin")
        .or(remotes.first())
        .map(|r| r.to_string())
}

/// Whether there are local commits the remote doesn't have.
fn has_unpushed_commits(path: &Path) -> bool {
    run_git(path, ["rev-list", "--count", "@{u}..HEAD"])
        .map(|count| count.trim() != "0")
        .unwrap_or(true)
}

fn merge_in_progress(path: &Path) -> bool {
    run_git(path, ["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok()
}

/// Syncs the repository with its remote: commits whatever is on disk and not committed yet, pulls (merging), and
/// pushes. Does nothing if `path` isn't a git repository with a remote.
///
/// Never leaves the repository in the middle of a merge: if the changes conflict, the merge is aborted and an error
/// returned, for the user to sort out with git.
///
/// Blocking, and may take a while (network): don't call from the UI thread.
pub fn sync(path: &Path) -> Result<(), String> {
    if !has_remote(path) {
        return Ok(());
    }

    // E.g. changes made with a text editor.
    commit_all(path).map_err(|err| err.message())?;

    match upstream(path) {
        Some(upstream) => {
            if let Err(err) = run_git(path, ["pull", "--no-rebase", "--no-edit"]) {
                if merge_in_progress(path) {
                    let _ = run_git(path, ["merge", "--abort"]);
                    return Err(format!(
                        "your changes and the ones in {upstream} conflict. Merge them with git, in {}",
                        path.display()
                    ));
                }
                return Err(err.message());
            }
            if has_unpushed_commits(path) {
                run_git(path, ["push"]).map_err(|err| err.message())?;
            }
        }
        None => {
            // Never pushed yet: push, and remember where to.
            let remote = default_remote(path).ok_or("no remote")?;
            run_git(path, ["push", "--set-upstream", remote.as_str(), "HEAD"]).map_err(|err| err.message())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaporeon-test-git-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git(path: &Path, args: &[&str]) {
        run_git(path, args).unwrap();
    }

    fn clone(remote: &Path, to: &Path) {
        git(to.parent().unwrap(), &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            to.to_str().unwrap(),
        ]);
        git(to, &["config", "user.email", "test@example.com"]);
        git(to, &["config", "user.name", "test"]);
    }

    /// A bare "remote" repository, and a clone of it, "a", with a first commit pushed.
    fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = temp_dir(name);
        let remote = dir.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let a = dir.join("a");
        clone(&remote, &a);
        std::fs::write(a.join("day.toml"), "one\n").unwrap();
        sync(&a).unwrap();
        (dir, remote, a)
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path.join("day.toml")).unwrap()
    }

    #[test]
    fn changes_go_both_ways() {
        let (dir, remote, a) = setup("both-ways");
        let b = dir.join("b");
        clone(&remote, &b);
        assert_eq!(read(&b), "one\n");

        std::fs::write(b.join("day.toml"), "one\ntwo\n").unwrap();
        sync(&b).unwrap();
        sync(&a).unwrap();
        assert_eq!(read(&a), "one\ntwo\n");

        // Changes to different lines merge.
        std::fs::write(a.join("day.toml"), "zero\none\ntwo\n").unwrap();
        std::fs::write(b.join("day.toml"), "one\ntwo\nthree\n").unwrap();
        sync(&a).unwrap();
        sync(&b).unwrap();
        sync(&a).unwrap();
        assert_eq!(read(&a), "zero\none\ntwo\nthree\n");
        assert_eq!(read(&b), "zero\none\ntwo\nthree\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn conflicts_are_reported_and_the_merge_aborted() {
        let (dir, remote, a) = setup("conflict");
        let b = dir.join("b");
        clone(&remote, &b);

        std::fs::write(a.join("day.toml"), "from a\n").unwrap();
        std::fs::write(b.join("day.toml"), "from b\n").unwrap();
        sync(&a).unwrap();
        let err = sync(&b).unwrap_err();
        assert!(err.contains("conflict"), "{err}");

        assert!(!merge_in_progress(&b));
        assert_eq!(read(&b), "from b\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nothing_happens_without_a_remote() {
        let dir = temp_dir("no-remote");
        assert!(sync(&dir).is_ok()); // Not even a repository.
        git(&dir, &["init", "-q"]);
        std::fs::write(dir.join("day.toml"), "one\n").unwrap();
        assert!(sync(&dir).is_ok());
        // Not committed either: without a remote, sync does nothing at all.
        assert!(run_git(&dir, ["rev-parse", "HEAD"]).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unreachable_remotes_fail_with_a_message() {
        let (dir, _remote, a) = setup("unreachable");
        git(&a, &[
            "remote",
            "set-url",
            "origin",
            dir.join("nowhere.git").to_str().unwrap(),
        ]);
        std::fs::write(a.join("day.toml"), "two\n").unwrap();
        let err = sync(&a).unwrap_err();
        assert!(!err.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
