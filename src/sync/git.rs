//! Git operations for sync functionality.
//!
//! This module provides a trait-based abstraction over git commands
//! to enable easy mocking in tests.

use miette::Diagnostic;
use std::path::Path;
use std::process::{Command, Output};
use thiserror::Error;

#[cfg(test)]
use mockall::automock;

/// Errors that can occur during git operations.
#[derive(Error, Diagnostic, Debug)]
pub enum GitError {
    #[error("Git command failed: {0}")]
    #[diagnostic(code(c5t::sync::git::command_failed))]
    CommandFailed(String),

    #[error("Git command returned non-zero exit code {code}: {stderr}")]
    #[diagnostic(code(c5t::sync::git::non_zero_exit))]
    NonZeroExit { code: i32, stderr: String },

    #[error("Git not installed or not in PATH")]
    #[diagnostic(code(c5t::sync::git::not_found))]
    GitNotFound,
}

/// Trait for git operations. Can be mocked in tests.
#[cfg_attr(test, automock)]
pub trait GitOps {
    /// Initialize a git repository at the given path.
    fn init(&self, path: &Path) -> Result<Output, GitError>;

    /// Add a remote to the repository.
    fn add_remote(&self, path: &Path, name: &str, url: &str) -> Result<Output, GitError>;

    /// Get the URL of a remote.
    fn remote_get_url(&self, path: &Path, name: &str) -> Result<Output, GitError>;

    /// Get repository status in porcelain format.
    fn status_porcelain(&self, path: &Path) -> Result<Output, GitError>;

    /// Add files to the staging area.
    fn add_files(&self, path: &Path, files: &[String]) -> Result<Output, GitError>;

    /// Create a commit with the given message.
    fn commit(&self, path: &Path, message: &str) -> Result<Output, GitError>;

    /// Pull from a remote repository.
    fn pull(&self, path: &Path, remote: &str, branch: &str) -> Result<Output, GitError>;

    /// Push to a remote repository.
    fn push(&self, path: &Path, remote: &str, branch: &str) -> Result<Output, GitError>;
}

/// Real implementation of GitOps using std::process::Command.
pub struct RealGit;

impl RealGit {
    pub fn new() -> Self {
        Self
    }

    /// Helper to run a git command and return the output.
    fn run_git(&self, path: &Path, args: &[&str]) -> Result<Output, GitError> {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    GitError::GitNotFound
                } else {
                    GitError::CommandFailed(e.to_string())
                }
            })
    }

    /// Check if the output indicates success, otherwise return an error.
    fn check_output(&self, output: Output) -> Result<Output, GitError> {
        if output.status.success() {
            Ok(output)
        } else {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(GitError::NonZeroExit { code, stderr })
        }
    }
}

impl Default for RealGit {
    fn default() -> Self {
        Self::new()
    }
}

impl GitOps for RealGit {
    fn init(&self, path: &Path) -> Result<Output, GitError> {
        let output = self.run_git(path, &["init"])?;
        self.check_output(output)
    }

    fn add_remote(&self, path: &Path, name: &str, url: &str) -> Result<Output, GitError> {
        let output = self.run_git(path, &["remote", "add", name, url])?;
        self.check_output(output)
    }

    fn remote_get_url(&self, path: &Path, name: &str) -> Result<Output, GitError> {
        let output = self.run_git(path, &["remote", "get-url", name])?;
        self.check_output(output)
    }

    fn status_porcelain(&self, path: &Path) -> Result<Output, GitError> {
        let output = self.run_git(path, &["status", "--porcelain"])?;
        self.check_output(output)
    }

    fn add_files(&self, path: &Path, files: &[String]) -> Result<Output, GitError> {
        let mut args = vec!["add"];
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        args.extend(file_refs);
        let output = self.run_git(path, &args)?;
        self.check_output(output)
    }

    fn commit(&self, path: &Path, message: &str) -> Result<Output, GitError> {
        let output = self.run_git(path, &["commit", "-m", message])?;
        self.check_output(output)
    }

    fn pull(&self, path: &Path, remote: &str, branch: &str) -> Result<Output, GitError> {
        let output = self.run_git(path, &["pull", remote, branch])?;
        self.check_output(output)
    }

    fn push(&self, path: &Path, remote: &str, branch: &str) -> Result<Output, GitError> {
        let output = self.run_git(path, &["push", remote, branch])?;
        self.check_output(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    /// Helper to create a mock Output
    fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn test_mock_git_init_success() {
        let mut mock = MockGitOps::new();

        mock.expect_init()
            .with(eq(Path::new("/tmp/test")))
            .times(1)
            .returning(|_| {
                Ok(mock_output(
                    0,
                    "Initialized empty Git repository in /tmp/test/.git/\n",
                    "",
                ))
            });

        let result = mock.init(Path::new("/tmp/test"));
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("Initialized"));
    }

    #[test]
    fn test_mock_git_init_failure() {
        let mut mock = MockGitOps::new();

        mock.expect_init()
            .with(eq(Path::new("/tmp/test")))
            .times(1)
            .returning(|_| Err(GitError::GitNotFound));

        let result = mock.init(Path::new("/tmp/test"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GitError::GitNotFound));
    }

    #[test]
    fn test_mock_status_clean() {
        let mut mock = MockGitOps::new();

        mock.expect_status_porcelain()
            .with(eq(Path::new("/tmp/test")))
            .times(1)
            .returning(|_| Ok(mock_output(0, "", "")));

        let result = mock.status_porcelain(Path::new("/tmp/test"));
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn test_mock_status_dirty() {
        let mut mock = MockGitOps::new();

        mock.expect_status_porcelain()
            .with(eq(Path::new("/tmp/test")))
            .times(1)
            .returning(|_| Ok(mock_output(0, " M repos.jsonl\n?? newfile.txt\n", "")));

        let result = mock.status_porcelain(Path::new("/tmp/test"));
        assert!(result.is_ok());

        let output = result.unwrap();
        let status = String::from_utf8_lossy(&output.stdout);
        assert!(status.contains("M repos.jsonl"));
        assert!(status.contains("?? newfile.txt"));
    }

    #[test]
    fn test_mock_commit_success() {
        let mut mock = MockGitOps::new();

        mock.expect_commit()
            .with(eq(Path::new("/tmp/test")), eq("Export data"))
            .times(1)
            .returning(|_, _| {
                Ok(mock_output(
                    0,
                    "[main abc1234] Export data\n 5 files changed, 42 insertions(+)\n",
                    "",
                ))
            });

        let result = mock.commit(Path::new("/tmp/test"), "Export data");
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_pull_up_to_date() {
        let mut mock = MockGitOps::new();

        mock.expect_pull()
            .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
            .times(1)
            .returning(|_, _, _| Ok(mock_output(0, "Already up to date.\n", "")));

        let result = mock.pull(Path::new("/tmp/test"), "origin", "main");
        assert!(result.is_ok());

        let output = result.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Already up to date"));
    }

    #[test]
    fn test_mock_push_success() {
        let mut mock = MockGitOps::new();

        mock.expect_push()
            .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
            .times(1)
            .returning(|_, _, _| {
                Ok(mock_output(
                    0,
                    "",
                    "To https://github.com/user/repo.git\n   abc1234..def5678  main -> main\n",
                ))
            });

        let result = mock.push(Path::new("/tmp/test"), "origin", "main");
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_network_error() {
        let mut mock = MockGitOps::new();

        mock.expect_pull()
            .with(eq(Path::new("/tmp/test")), eq("origin"), eq("main"))
            .times(1)
            .returning(|_, _, _| {
                Err(GitError::NonZeroExit {
                    code: 128,
                    stderr: "fatal: unable to access 'https://...': Could not resolve host\n"
                        .to_string(),
                })
            });

        let result = mock.pull(Path::new("/tmp/test"), "origin", "main");
        assert!(result.is_err());

        if let Err(GitError::NonZeroExit { code, stderr }) = result {
            assert_eq!(code, 128);
            assert!(stderr.contains("Could not resolve host"));
        } else {
            panic!("Expected NonZeroExit error");
        }
    }
}
