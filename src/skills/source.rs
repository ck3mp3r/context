//! Source protocol detection and fetching
//!
//! This module handles parsing source URLs and fetching skill sources from:
//! - Git repositories (git+https, git+ssh)
//! - HTTP/HTTPS archives (.zip, .tar.gz)
//! - Local filesystem paths (file://, relative, absolute)

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
    /// Local filesystem path
    LocalPath { path: PathBuf },
    /// file:// URI
    FileUri { path: PathBuf },
    /// HTTP/HTTPS archive (.zip, .tar.gz, etc.)
    HttpArchive { url: String, extension: String },
}

/// Parse a source URL/path string into a SourceType
///
/// Examples:
/// - `git+https://github.com/user/repo/path/to/skill` → GitHttps with path
/// - `git+ssh://git@github.com/user/repo` → GitSsh
/// - `file:///absolute/path` → FileUri
/// - `/absolute/path` → LocalPath
/// - `./relative/path` → LocalPath
/// - `https://example.com/skill.zip` → HttpArchive
pub fn parse_source(source: &str) -> Result<SourceType, SourceError> {
    // TODO: Implement source parsing
    // 1. Check for git+https:// or git+ssh:// prefix
    // 2. Check for file:// prefix
    // 3. Check for https:// or http:// (archive)
    // 4. Check if path exists locally (relative or absolute)
    // 5. Return error if no match

    Err(SourceError::InvalidUrl(
        "Source parsing not yet implemented".to_string(),
    ))
}

/// Fetch source to a temporary directory
///
/// Returns the path to the temporary directory containing the fetched source.
/// Caller is responsible for cleanup.
pub fn fetch_source(_source_type: SourceType) -> Result<PathBuf, SourceError> {
    // TODO: Implement source fetching
    // - For Git: clone to temp dir
    // - For HTTP: download and extract to temp dir
    // - For local paths: verify exists and return path (no copy needed)

    Err(SourceError::UnsupportedProtocol(
        "Source fetching not yet implemented".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_https_with_path() {
        let result = parse_source("git+https://github.com/user/repo/skills/rust");
        assert!(result.is_err()); // Placeholder until implemented
    }

    #[test]
    fn test_parse_local_path() {
        let result = parse_source("/tmp/skills/rust");
        assert!(result.is_err()); // Placeholder until implemented
    }
}
