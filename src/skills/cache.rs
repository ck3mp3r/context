//! Cache management for skill attachments.
//!
//! Skills can have attachments stored as base64-encoded content in the database.
//! When a skill is loaded via `c5t_get_skill`, attachments are extracted to a local
//! cache directory preserving the exact directory structure from the source.
//!
//! Cache structure (example from docx skill):
//! ```text
//! ~/.local/share/c5t-dev/skills/<skill-id>/  (debug builds)
//! ~/.local/share/c5t/skills/<skill-id>/      (release builds)
//!   ├── docx-js.md
//!   ├── ooxml.md
//!   ├── scripts/
//!   │   ├── __init__.py
//!   │   ├── document.py
//!   │   └── utilities.py
//!   └── ooxml/
//!       └── document.xml
//! ```

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::db::{DbError, SkillAttachment};
use crate::sync::get_data_dir;

/// Get the base cache directory for skills.
/// Returns: `~/.local/share/c5t-dev/skills/` (debug) or `~/.local/share/c5t/skills/` (release)
pub fn get_skills_cache_dir() -> PathBuf {
    get_data_dir().join("skills")
}

/// Get the cache directory for a specific skill.
/// Returns: `~/.local/share/c5t-dev/skills/<skill-id>/` (debug) or `~/.local/share/c5t/skills/<skill-id>/` (release)
///
/// # Arguments
/// * `skill_id` - Skill ID
pub fn get_skill_cache_dir(skill_id: &str) -> PathBuf {
    get_skills_cache_dir().join(skill_id)
}

/// Extract skill attachments to cache.
///
/// Preserves the exact directory structure from the source skill.
/// The `filename` field contains relative paths (e.g., "docx-js.md", "scripts/__init__.py", "ooxml/document.xml").
///
/// For shell scripts (.sh, .bash), sets executable permissions.
///
/// # Arguments
/// * `skill_id` - Skill ID
/// * `attachments` - List of attachments to extract
///
/// # Returns
/// Path to skill cache directory
pub fn extract_attachments(
    skill_id: &str,
    attachments: &[SkillAttachment],
) -> Result<PathBuf, DbError> {
    let cache_dir = get_skill_cache_dir(skill_id);

    // Extract each attachment
    for attachment in attachments {
        // filename is a relative path like "docx-js.md" or "scripts/__init__.py"
        let file_path = cache_dir.join(&attachment.filename);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| DbError::Database {
                message: format!("Failed to create directory {}: {}", parent.display(), e),
            })?;
        }

        // Decode base64 content
        let content = BASE64
            .decode(&attachment.content)
            .map_err(|e| DbError::Database {
                message: format!(
                    "Failed to decode base64 content for {}: {}",
                    attachment.filename, e
                ),
            })?;

        // Write to file
        let mut file = fs::File::create(&file_path).map_err(|e| DbError::Database {
            message: format!("Failed to create file {}: {}", file_path.display(), e),
        })?;
        file.write_all(&content).map_err(|e| DbError::Database {
            message: format!("Failed to write file {}: {}", file_path.display(), e),
        })?;

        // Set executable permissions for shell scripts
        #[cfg(unix)]
        if attachment.filename.ends_with(".sh") || attachment.filename.ends_with(".bash") {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path)
                .map_err(|e| DbError::Database {
                    message: format!(
                        "Failed to read file metadata for {}: {}",
                        file_path.display(),
                        e
                    ),
                })?
                .permissions();
            perms.set_mode(0o755); // rwxr-xr-x
            fs::set_permissions(&file_path, perms).map_err(|e| DbError::Database {
                message: format!(
                    "Failed to set executable permissions for {}: {}",
                    file_path.display(),
                    e
                ),
            })?;
        }
    }

    Ok(cache_dir)
}

/// Invalidate (clear) the cache for a specific skill.
///
/// Removes all cached attachments for the skill. Called when:
/// - Skill is updated
/// - Skill is deleted
/// - Attachments are modified
///
/// # Arguments
/// * `skill_id` - Skill ID to invalidate cache for
pub fn invalidate_cache(skill_id: &str) -> Result<(), DbError> {
    let cache_dir = get_skill_cache_dir(skill_id);

    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).map_err(|e| DbError::Database {
            message: format!(
                "Failed to remove cache directory {}: {}",
                cache_dir.display(),
                e
            ),
        })?;
    }

    Ok(())
}

/// Clear all skill caches.
///
/// Removes the entire skills cache directory. Useful for cleanup or troubleshooting.
pub fn clear_all_caches() -> Result<(), DbError> {
    let cache_dir = get_skills_cache_dir();

    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir).map_err(|e| DbError::Database {
            message: format!(
                "Failed to remove cache directory {}: {}",
                cache_dir.display(),
                e
            ),
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_skills_cache_dir() {
        let cache_dir = get_skills_cache_dir();
        // Should use XDG data dir + skills
        assert!(cache_dir.to_string_lossy().contains("skills"));
        assert!(cache_dir.to_string_lossy().contains("c5t"));
    }

    #[test]
    fn test_get_skill_cache_dir() {
        let cache_dir = get_skill_cache_dir("abc12345");
        // Should use XDG data dir + skills/<id>
        assert!(cache_dir.to_string_lossy().contains("skills/abc12345"));
        assert!(cache_dir.to_string_lossy().contains("c5t"));
    }

    #[test]
    fn test_extract_attachments() {
        use crate::db::utils::generate_entity_id;
        use crate::sync::set_base_path;

        let skill_id = generate_entity_id();

        // Use a temp directory for testing - unique per test invocation
        let unique_id = generate_entity_id();
        let temp_base = std::env::temp_dir().join(format!("test-cache-{}", unique_id));
        set_base_path(temp_base.clone());

        // Create test attachments with relative paths (like real scanner output)
        let script_content = "#!/bin/bash\necho 'Hello'";
        let reference_content = "# Documentation\n\nThis is a reference.";
        let nested_content = "<xml/>";

        let attachments = vec![
            SkillAttachment {
                id: generate_entity_id(),
                skill_id: skill_id.clone(),
                type_: "script".to_string(),
                filename: "scripts/test.sh".to_string(), // Relative path
                content: BASE64.encode(script_content),
                content_hash: "abc123".to_string(),
                mime_type: Some("text/x-shellscript".to_string()),
                created_at: None,
                updated_at: None,
            },
            SkillAttachment {
                id: generate_entity_id(),
                skill_id: skill_id.clone(),
                type_: "reference".to_string(),
                filename: "README.md".to_string(), // Root-level file
                content: BASE64.encode(reference_content),
                content_hash: "def456".to_string(),
                mime_type: Some("text/markdown".to_string()),
                created_at: None,
                updated_at: None,
            },
            SkillAttachment {
                id: generate_entity_id(),
                skill_id: skill_id.clone(),
                type_: "reference".to_string(),
                filename: "ooxml/document.xml".to_string(), // Nested path
                content: BASE64.encode(nested_content),
                content_hash: "ghi789".to_string(),
                mime_type: Some("application/xml".to_string()),
                created_at: None,
                updated_at: None,
            },
        ];

        // Extract attachments to temp directory
        let cache_dir = extract_attachments(&skill_id, &attachments).unwrap();

        // Verify files exist at correct paths
        assert!(cache_dir.join("scripts/test.sh").exists());
        assert!(cache_dir.join("README.md").exists());
        assert!(cache_dir.join("ooxml/document.xml").exists());

        // Verify content
        let script_read = fs::read_to_string(cache_dir.join("scripts/test.sh")).unwrap();
        assert_eq!(script_read, script_content);

        let reference_read = fs::read_to_string(cache_dir.join("README.md")).unwrap();
        assert_eq!(reference_read, reference_content);

        let nested_read = fs::read_to_string(cache_dir.join("ooxml/document.xml")).unwrap();
        assert_eq!(nested_read, nested_content);

        // Verify executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script_meta = fs::metadata(cache_dir.join("scripts/test.sh")).unwrap();
            let perms = script_meta.permissions();
            assert_eq!(perms.mode() & 0o111, 0o111); // Check executable bits
        }

        // Cleanup temp directory
        invalidate_cache(&skill_id).unwrap();
        assert!(!cache_dir.exists());

        // Clean up temp base directory
        let _ = std::fs::remove_dir_all(&temp_base);

        // Clear the global base path for other tests
        crate::sync::clear_base_path();
    }

    #[test]
    fn test_invalidate_cache() {
        use crate::db::utils::generate_entity_id;
        use crate::sync::set_base_path;

        let skill_id = generate_entity_id();

        // Use unique temp directory per test invocation
        let unique_id = generate_entity_id();
        let temp_base = std::env::temp_dir().join(format!("test-cache-invalidate-{}", unique_id));
        set_base_path(temp_base.clone());
        let cache_dir = get_skill_cache_dir(&skill_id);

        // Create cache directory
        fs::create_dir_all(&cache_dir).unwrap();
        assert!(cache_dir.exists());

        // Invalidate
        invalidate_cache(&skill_id).unwrap();
        assert!(!cache_dir.exists());

        // Invalidating non-existent cache should not error
        invalidate_cache(&skill_id).unwrap();

        // Clean up temp base directory
        let _ = std::fs::remove_dir_all(&temp_base);

        // Clear the global base path for other tests
        crate::sync::clear_base_path();
    }
}
