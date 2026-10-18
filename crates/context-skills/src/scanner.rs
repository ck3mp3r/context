//! Attachment scanning and encoding
//!
//! Scans a skill directory recursively for all files and classifies them by extension.
//! Preserves directory structure for cache extraction.

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

    /// Relative path from skill directory (preserves directory structure)
    /// Examples: "docx-js.md", "scripts/__init__.py", "ooxml/document.xml"
    pub filename: String,

    /// Base64-encoded file content
    pub content_base64: String,

    /// SHA256 hash of file content
    pub content_hash: String,

    /// MIME type (if detectable)
    pub mime_type: Option<String>,
}

/// Scan a skill directory recursively for all files
///
/// Real-world skills have varied structures:
/// - Files at root level (pdf/reference.md, docx/docx-js.md)
/// - Subdirectories with any name (scripts/, reference/, ooxml/)
/// - No attachments at all (brand-guidelines)
///
/// Files are classified by extension:
/// - Scripts: .py, .sh, .bash, .js, .rb, etc.
/// - References: .md, .txt, .json, .yaml, .xml
/// - Assets: .png, .jpg, .svg, .gif
///
/// Skips: SKILL.md, LICENSE*, .git*, README*
///
/// Returns a list of all found attachments with their content encoded.
pub fn scan_attachments(base_dir: &Path) -> Result<Vec<AttachmentData>, ScannerError> {
    let mut attachments = Vec::new();
    scan_directory_recursive(base_dir, base_dir, &mut attachments)?;
    Ok(attachments)
}

/// Recursively scan directory and collect attachments
fn scan_directory_recursive(
    base_dir: &Path,
    current_dir: &Path,
    attachments: &mut Vec<AttachmentData>,
) -> Result<(), ScannerError> {
    let entries = std::fs::read_dir(current_dir)
        .map_err(|e| ScannerError::ReadDirError(format!("{}: {}", current_dir.display(), e)))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| ScannerError::ReadDirError(format!("Failed to read entry: {}", e)))?;

        let path = entry.path();
        let file_name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| {
                ScannerError::FileSystemError(format!("Invalid filename: {}", path.display()))
            })?
            .to_string();

        // Skip ignored files
        if should_skip(&file_name) {
            continue;
        }

        if path.is_dir() {
            // Recursively scan subdirectory
            scan_directory_recursive(base_dir, &path, attachments)?;
        } else if path.is_file() {
            // Process file
            let relative_path = path
                .strip_prefix(base_dir)
                .map_err(|e| {
                    ScannerError::FileSystemError(format!("Failed to compute relative path: {}", e))
                })?
                .to_str()
                .ok_or_else(|| {
                    ScannerError::FileSystemError(format!("Non-UTF8 path: {}", path.display()))
                })?
                .to_string();

            // Classify file by extension
            let type_ = classify_file(&file_name);

            // Read file content
            let content = std::fs::read(&path)
                .map_err(|e| ScannerError::ReadFileError(format!("{}: {}", path.display(), e)))?;

            // Compute SHA256 hash
            let content_hash = sha256_hash(&content);

            // Base64 encode
            let content_base64 = base64_encode(&content);

            // Detect MIME type
            let mime_type = detect_mime_type(&file_name);

            attachments.push(AttachmentData {
                type_,
                filename: relative_path,
                content_base64,
                content_hash,
                mime_type,
            });
        }
    }

    Ok(())
}

/// Check if file should be skipped during scanning
fn should_skip(filename: &str) -> bool {
    matches!(
        filename,
        "SKILL.md"
            | "LICENSE"
            | "LICENSE.txt"
            | "LICENSE.md"
            | "README"
            | "README.md"
            | "README.txt"
            | ".git"
            | ".gitignore"
            | ".gitattributes"
            | ".DS_Store"
    ) || filename.starts_with('.')
}

/// Classify file by extension into script, reference, or asset
fn classify_file(filename: &str) -> String {
    let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    match extension.as_str() {
        // Scripts
        "py" | "sh" | "bash" | "js" | "rb" | "pl" | "php" => "script".to_string(),
        // References (documentation, config, data)
        "md" | "markdown" | "txt" | "json" | "yaml" | "yml" | "xml" | "toml" | "ini" | "csv" => {
            "reference".to_string()
        }
        // Assets (images, binary files)
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "pdf" | "zip" | "tar" | "gz" => {
            "asset".to_string()
        }
        // Default to reference for unknown text files
        _ => "reference".to_string(),
    }
}

/// Compute SHA256 hash of data
fn sha256_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        write!(acc, "{:02x}", b).unwrap();
        acc
    })
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
#[path = "../test/scanner.rs"]
mod scanner_test;
