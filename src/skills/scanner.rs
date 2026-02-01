//! Attachment scanning and encoding
//!
//! Scans a skill directory for attachments (scripts, references, assets)
//! and encodes them to base64 for database storage.

use base64::Engine as _;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScannerError {
    #[error("Failed to read directory: {0}")]
    ReadDirError(String),

    #[error("Failed to read file: {0}")]
    ReadFileError(String),

    #[error("Invalid attachment type: {0}")]
    InvalidType(String),

    #[error("File system error: {0}")]
    FileSystemError(String),
}

/// Represents an attachment found during scanning
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used when import is implemented
pub struct AttachmentData {
    /// Attachment type: "script", "reference", or "asset"
    pub type_: String,

    /// Filename (relative to type directory)
    pub filename: String,

    /// Base64-encoded file content
    pub content_base64: String,

    /// SHA256 hash of file content
    pub content_hash: String,

    /// MIME type (if detectable)
    pub mime_type: Option<String>,
}

/// Scan a skill directory for attachments
///
/// Expected directory structure:
/// ```text
/// skill_dir/
///   SKILL.md (not scanned - parsed separately)
///   scripts/
///     setup.sh
///     helper.py
///   references/
///     docs.md
///     api.json
///   assets/
///     logo.png
///     diagram.svg
/// ```
///
/// Returns a list of all found attachments with their content encoded.
pub fn scan_attachments(base_dir: &Path) -> Result<Vec<AttachmentData>, ScannerError> {
    let mut attachments = Vec::new();

    // Scan each attachment type directory
    for (type_name, subdir) in [
        ("script", "scripts"),
        ("reference", "references"),
        ("asset", "assets"),
    ] {
        let dir_path = base_dir.join(subdir);

        // Skip if directory doesn't exist
        if !dir_path.exists() {
            continue;
        }

        // Read directory entries
        let entries = std::fs::read_dir(&dir_path)
            .map_err(|e| ScannerError::ReadDirError(format!("{}: {}", dir_path.display(), e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| ScannerError::ReadDirError(format!("Failed to read entry: {}", e)))?;

            let path = entry.path();

            // Skip directories (only process files)
            if !path.is_file() {
                continue;
            }

            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    ScannerError::FileSystemError(format!("Invalid filename: {}", path.display()))
                })?
                .to_string();

            // Read file content
            let content = std::fs::read(&path)
                .map_err(|e| ScannerError::ReadFileError(format!("{}: {}", path.display(), e)))?;

            // Compute SHA256 hash
            let content_hash = sha256_hash(&content);

            // Base64 encode
            let content_base64 = base64_encode(&content);

            // Detect MIME type
            let mime_type = detect_mime_type(&filename);

            attachments.push(AttachmentData {
                type_: type_name.to_string(),
                filename,
                content_base64,
                content_hash,
                mime_type,
            });
        }
    }

    Ok(attachments)
}

/// Compute SHA256 hash of data
fn sha256_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Detect MIME type from filename
fn detect_mime_type(filename: &str) -> Option<String> {
    // Simple extension-based detection
    let extension = filename.rsplit('.').next()?;

    let mime = match extension.to_lowercase().as_str() {
        "sh" | "bash" => "text/x-shellscript",
        "py" => "text/x-python",
        "js" => "application/javascript",
        "rb" => "text/x-ruby",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "yaml" | "yml" => "application/x-yaml",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => return None,
    };

    Some(mime.to_string())
}

/// Base64 encode data
fn base64_encode(data: &[u8]) -> String {
    base64::prelude::BASE64_STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scan_empty_directory() {
        let path = PathBuf::from("/tmp/nonexistent");
        let result = scan_attachments(&path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty()); // Placeholder behavior
    }
}
