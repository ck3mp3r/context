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

    #[test]
    fn test_scan_skill_with_root_level_files() {
        // Create temp skill directory with files at root level (like docx skill)
        let temp_dir = std::env::temp_dir().join(format!("test-skill-root-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create SKILL.md (should be skipped)
        std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();

        // Create root-level reference files (like docx/docx-js.md)
        std::fs::write(temp_dir.join("reference.md"), "# Reference").unwrap();
        std::fs::write(temp_dir.join("forms.json"), r#"{"test": true}"#).unwrap();

        // Create LICENSE (should be skipped)
        std::fs::write(temp_dir.join("LICENSE.txt"), "Apache").unwrap();

        let result = scan_attachments(&temp_dir).unwrap();

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();

        // Should find 2 files (reference.md, forms.json), skip SKILL.md and LICENSE.txt
        assert_eq!(result.len(), 2);

        let filenames: Vec<&str> = result.iter().map(|a| a.filename.as_str()).collect();
        assert!(filenames.contains(&"reference.md"));
        assert!(filenames.contains(&"forms.json"));

        // Both should be classified as references
        assert!(result.iter().all(|a| a.type_ == "reference"));
    }

    #[test]
    fn test_scan_skill_with_subdirectories() {
        // Create temp skill directory with subdirectories (like pdf skill)
        let temp_dir =
            std::env::temp_dir().join(format!("test-skill-subdir-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create SKILL.md
        std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();

        // Create scripts directory
        std::fs::create_dir_all(temp_dir.join("scripts")).unwrap();
        std::fs::write(temp_dir.join("scripts/__init__.py"), "# Init").unwrap();
        std::fs::write(temp_dir.join("scripts/helper.py"), "# Helper").unwrap();

        // Create custom directory (like ooxml/)
        std::fs::create_dir_all(temp_dir.join("ooxml")).unwrap();
        std::fs::write(temp_dir.join("ooxml/document.xml"), "<xml/>").unwrap();

        let result = scan_attachments(&temp_dir).unwrap();

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();

        // Should find 3 files with preserved paths
        assert_eq!(result.len(), 3);

        let filenames: Vec<&str> = result.iter().map(|a| a.filename.as_str()).collect();
        assert!(filenames.contains(&"scripts/__init__.py"));
        assert!(filenames.contains(&"scripts/helper.py"));
        assert!(filenames.contains(&"ooxml/document.xml"));

        // Check types
        let scripts: Vec<_> = result.iter().filter(|a| a.type_ == "script").collect();
        assert_eq!(scripts.len(), 2);

        let references: Vec<_> = result.iter().filter(|a| a.type_ == "reference").collect();
        assert_eq!(references.len(), 1);
    }

    #[test]
    fn test_scan_skill_no_attachments() {
        // Create temp skill directory with only SKILL.md (like brand-guidelines)
        let temp_dir =
            std::env::temp_dir().join(format!("test-skill-empty-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("SKILL.md"), "# Test").unwrap();
        std::fs::write(temp_dir.join("LICENSE.txt"), "Apache").unwrap();

        let result = scan_attachments(&temp_dir).unwrap();

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();

        // Should find nothing (SKILL.md and LICENSE skipped)
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_classify_file() {
        assert_eq!(classify_file("test.py"), "script");
        assert_eq!(classify_file("test.sh"), "script");
        assert_eq!(classify_file("test.js"), "script");

        assert_eq!(classify_file("test.md"), "reference");
        assert_eq!(classify_file("test.json"), "reference");
        assert_eq!(classify_file("test.yaml"), "reference");

        assert_eq!(classify_file("test.png"), "asset");
        assert_eq!(classify_file("test.jpg"), "asset");
        assert_eq!(classify_file("test.svg"), "asset");
    }

    #[test]
    fn test_should_skip() {
        assert!(should_skip("SKILL.md"));
        assert!(should_skip("LICENSE"));
        assert!(should_skip("LICENSE.txt"));
        assert!(should_skip("README.md"));
        assert!(should_skip(".git"));
        assert!(should_skip(".gitignore"));
        assert!(should_skip(".DS_Store"));

        assert!(!should_skip("reference.md"));
        assert!(!should_skip("test.py"));
    }
}
