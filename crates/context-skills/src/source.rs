//! Source protocol detection and fetching
//!
//! This module handles parsing source URLs and fetching skill sources from:
//! - Git repositories (git+https, git+ssh)
//! - Local filesystem paths (file://, relative, absolute)
//!
//! NOTE: HTTP/HTTPS archives not yet supported

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("Invalid source URL: {0}")]
    InvalidUrl(String),

    #[error("Unsupported protocol: {0}")]
    UnsupportedProtocol(String),

    #[error("Git operation failed: {0}")]
    GitError(String),

    #[error("Download failed: {0}")]
    DownloadError(String),

    #[error("Archive extraction failed: {0}")]
    ExtractionError(String),

    #[error("File system error: {0}")]
    FileSystemError(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Represents the type of skill source
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum SourceType {
    /// Git repository via HTTPS with optional subpath
    GitHttps {
        repo_url: String,
        path: Option<String>,
    },
    /// Git repository via SSH with optional subpath
    GitSsh {
        repo_url: String,
        path: Option<String>,
    },
    /// Local filesystem path (file://, /absolute, or ./relative)
    LocalPath { path: PathBuf },
}

/// Parse a source URL string into a SourceType
///
/// Supported formats:
/// - `git+https://github.com/user/repo/path/to/skill` → GitHttps with path
/// - `git+ssh://git@github.com/user/repo` → GitSsh
/// - `file:///absolute/path` → LocalPath
/// - `/absolute/path` → LocalPath (if exists)
/// - `./relative/path` → LocalPath (if exists)
///
/// The URL format embeds the subpath after the repo:
/// `git+https://domain/org/repo/path/to/skill`
///                    ^^^^^^^^^^^^^^^^ ^^^^^^^^^^^^^^
///                    repo URL         subpath (optional)
#[allow(dead_code)]
pub fn parse_source(source: &str) -> Result<SourceType, SourceError> {
    let source = source.trim();

    // 1. Check for git+https://
    if source.starts_with("git+https://") {
        return parse_git_url(source, "git+https://", false);
    }

    // 2. Check for git+ssh://
    if source.starts_with("git+ssh://") {
        return parse_git_url(source, "git+ssh://", true);
    }

    // 3. Check for file:// URI (strip prefix and treat as local path)
    if source.starts_with("file://") {
        let path = source.trim_start_matches("file://");
        return Ok(SourceType::LocalPath {
            path: PathBuf::from(path),
        });
    }

    // 4. Check if it's a local path (relative or absolute)
    let path = PathBuf::from(source);
    if path.exists() {
        return Ok(SourceType::LocalPath { path });
    }

    // No match
    Err(SourceError::InvalidUrl(format!(
        "Unsupported source: '{}'. Supported: git+https://, git+ssh://, file://, or existing local path",
        source
    )))
}

/// Parse git+https:// or git+ssh:// URL
///
/// Format: git+https://github.com/user/repo/path/to/skill
///         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^----^^^^^^^^^^^
///         prefix     repo_url                 subpath (optional)
#[allow(dead_code)]
fn parse_git_url(source: &str, prefix: &str, is_ssh: bool) -> Result<SourceType, SourceError> {
    let without_prefix = source.trim_start_matches(prefix);

    // Split by '/' to find components
    let parts: Vec<&str> = without_prefix.split('/').collect();

    // Need at least domain/org/repo (3 parts)
    if parts.len() < 3 {
        return Err(SourceError::InvalidUrl(format!(
            "Invalid git URL format: '{}'. Expected: {}domain/org/repo[/path]",
            source, prefix
        )));
    }

    // Reconstruct repo URL: prefix + domain/org/repo
    let repo_url = format!("{}{}/{}/{}", prefix, parts[0], parts[1], parts[2]);

    // Extract optional subpath (everything after domain/org/repo)
    let subpath = if parts.len() > 3 {
        Some(parts[3..].join("/"))
    } else {
        None
    };

    if is_ssh {
        Ok(SourceType::GitSsh {
            repo_url,
            path: subpath,
        })
    } else {
        Ok(SourceType::GitHttps {
            repo_url,
            path: subpath,
        })
    }
}

/// Fetch source to a temporary directory
///
/// For Git sources: clones to a temp directory and returns the path.
/// For local paths: validates existence and returns the original path (no copy).
///
/// Returns the path to the directory containing the skill source.
/// Temp directories are created with predictable names: /tmp/c5t-skill-import-{pid}
/// Caller is responsible for cleanup using the same pattern.
#[allow(dead_code)] // Used when import is implemented
pub fn fetch_source(source_type: SourceType) -> Result<PathBuf, SourceError> {
    match source_type {
        SourceType::GitHttps { repo_url, path } | SourceType::GitSsh { repo_url, path } => {
            // Create temp directory for git clone
            let temp_dir =
                std::env::temp_dir().join(format!("c5t-skill-import-{}", std::process::id()));

            // Remove existing temp dir if it exists (from previous failed import)
            if temp_dir.exists() {
                std::fs::remove_dir_all(&temp_dir).map_err(|e| {
                    SourceError::FileSystemError(format!(
                        "Failed to remove existing temp directory: {}",
                        e
                    ))
                })?;
            }

            std::fs::create_dir_all(&temp_dir).map_err(|e| {
                SourceError::FileSystemError(format!("Failed to create temp directory: {}", e))
            })?;

            // Strip git+ prefix for actual git clone
            let git_url = repo_url
                .replace("git+https://", "https://")
                .replace("git+ssh://", "ssh://");

            // Execute git clone
            let output = std::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1") // Shallow clone for faster fetch
                .arg(&git_url)
                .arg(&temp_dir)
                .output()
                .map_err(|e| SourceError::GitError(format!("Failed to execute git: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Cleanup temp dir on git failure
                std::fs::remove_dir_all(&temp_dir).ok();
                return Err(SourceError::GitError(format!(
                    "Git clone failed: {}",
                    stderr
                )));
            }

            // Navigate to subpath if specified
            let final_path = if let Some(subpath) = path {
                temp_dir.join(subpath)
            } else {
                temp_dir
            };

            Ok(final_path)
        }
        SourceType::LocalPath { path } => {
            // Verify path exists
            if !path.exists() {
                return Err(SourceError::InvalidPath(format!(
                    "Path does not exist: {}",
                    path.display()
                )));
            }

            // Verify it's a directory
            if !path.is_dir() {
                return Err(SourceError::InvalidPath(format!(
                    "Path is not a directory: {}",
                    path.display()
                )));
            }

            // Return the path as-is (no copy needed)
            Ok(path)
        }
    }
}

#[cfg(test)]
#[path = "../test/source.rs"]
mod source_test;
