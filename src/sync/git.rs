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

    #[error("Git command returned non-zero exit code {code}: {output}")]
    #[diagnostic(code(c5t::sync::git::non_zero_exit))]
    NonZeroExit { code: i32, output: String },

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
#[derive(Clone, Copy)]
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
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            // Combine stdout and stderr for error message
            let combined = if !stdout.is_empty() && !stderr.is_empty() {
                format!("{}\n{}", stdout, stderr)
            } else if !stdout.is_empty() {
                stdout
            } else {
                stderr
            };
            Err(GitError::NonZeroExit {
                code,
                output: combined,
            })
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
