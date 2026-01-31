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
/// For Git sources, caller is responsible for cleanup of temp directory.
#[allow(dead_code)] // Used when import is implemented
pub fn fetch_source(source_type: SourceType) -> Result<PathBuf, SourceError> {
    match source_type {
        SourceType::GitHttps { repo_url, .. } | SourceType::GitSsh { repo_url, .. } => {
            // Create temp directory for git clone
            let temp_dir =
                std::env::temp_dir().join(format!("c5t-skill-import-{}", std::process::id()));
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
                return Err(SourceError::GitError(format!(
                    "Git clone failed: {}",
                    stderr
                )));
            }

            Ok(temp_dir)
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
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_https_simple() {
        let result = parse_source("git+https://github.com/user/repo");
        assert!(result.is_ok());
        match result.unwrap() {
            SourceType::GitHttps { repo_url, path } => {
                assert_eq!(repo_url, "git+https://github.com/user/repo");
                assert_eq!(path, None);
            }
            _ => panic!("Expected GitHttps variant"),
        }
    }

    #[test]
    fn test_parse_git_https_with_path() {
        let result = parse_source("git+https://github.com/user/repo/skills/rust");
        assert!(result.is_ok());
        match result.unwrap() {
            SourceType::GitHttps { repo_url, path } => {
                assert_eq!(repo_url, "git+https://github.com/user/repo");
                assert_eq!(path, Some("skills/rust".to_string()));
            }
            _ => panic!("Expected GitHttps variant"),
        }
    }

    #[test]
    fn test_parse_git_ssh() {
        let result = parse_source("git+ssh://git@github.com/user/repo");
        assert!(result.is_ok());
        match result.unwrap() {
            SourceType::GitSsh { repo_url, path } => {
                assert_eq!(repo_url, "git+ssh://git@github.com/user/repo");
                assert_eq!(path, None);
            }
            _ => panic!("Expected GitSsh variant"),
        }
    }

    #[test]
    fn test_parse_file_uri() {
        let result = parse_source("file:///tmp/skill");
        assert!(result.is_ok());
        match result.unwrap() {
            SourceType::LocalPath { path } => {
                assert_eq!(path, PathBuf::from("/tmp/skill"));
            }
            _ => panic!("Expected LocalPath variant"),
        }
    }

    #[test]
    fn test_parse_local_path() {
        // Create a temp file to test with
        let temp = std::env::temp_dir();
        let result = parse_source(temp.to_str().unwrap());
        assert!(result.is_ok());
        match result.unwrap() {
            SourceType::LocalPath { path } => {
                assert_eq!(path, temp);
            }
            _ => panic!("Expected LocalPath variant"),
        }
    }

    #[test]
    fn test_parse_invalid_git_url() {
        let result = parse_source("git+https://github.com/user");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_nonexistent_path() {
        let result = parse_source("/this/path/does/not/exist");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unsupported_protocol() {
        let result = parse_source("https://example.com/skill.zip");
        assert!(result.is_err());
    }
}
