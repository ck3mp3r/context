//! Cache management for skill attachments.
//!
//! Skills can have attachments (scripts, references, assets) stored as base64-encoded
//! content in the database. When a skill is loaded via `c5t_get_skill`, attachments
//! are extracted to a local cache directory for agent execution.
//!
//! Cache structure:
//! ```text
//! .context/cache/skills/<skill-id>/
//!   ├── scripts/
//!   │   ├── deploy.sh      (executable if .sh/.bash)
//!   │   └── rollback.sh
//!   ├── references/
//!   │   └── architecture.md
//!   └── assets/
//!       └── diagram.png
//! ```

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::db::{DbError, SkillAttachment};

/// Get the base cache directory for skills.
/// Returns: `.context/cache/skills/`
pub fn get_skills_cache_dir() -> PathBuf {
    PathBuf::from(".context").join("cache").join("skills")
}

/// Get the cache directory for a specific skill.
/// Returns: `.context/cache/skills/<skill-id>/`
pub fn get_skill_cache_dir(skill_id: &str) -> PathBuf {
    get_skills_cache_dir().join(skill_id)
}

/// Extract skill attachments to cache.
///
/// Creates cache directory structure and writes all attachments organized by type:
/// - scripts/ → type='script'
/// - references/ → type='reference'
/// - assets/ → type='asset'
///
/// For scripts with .sh or .bash extensions, sets executable permissions.
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

    // Create base cache directory and type subdirectories
    fs::create_dir_all(cache_dir.join("scripts")).map_err(|e| DbError::Database {
        message: format!("Failed to create scripts cache directory: {}", e),
    })?;
    fs::create_dir_all(cache_dir.join("references")).map_err(|e| DbError::Database {
        message: format!("Failed to create references cache directory: {}", e),
    })?;
    fs::create_dir_all(cache_dir.join("assets")).map_err(|e| DbError::Database {
        message: format!("Failed to create assets cache directory: {}", e),
    })?;

    // Extract each attachment
    for attachment in attachments {
        let subdir = match attachment.type_.as_str() {
            "script" => "scripts",
            "reference" => "references",
            "asset" => "assets",
            _ => continue, // Skip unknown types
        };

        let file_path = cache_dir.join(subdir).join(&attachment.filename);

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
        if attachment.type_ == "script"
            && (attachment.filename.ends_with(".sh") || attachment.filename.ends_with(".bash"))
        {
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
        assert_eq!(cache_dir, PathBuf::from(".context/cache/skills"));
    }

    #[test]
    fn test_get_skill_cache_dir() {
        let cache_dir = get_skill_cache_dir("abc12345");
        assert_eq!(cache_dir, PathBuf::from(".context/cache/skills/abc12345"));
    }

    #[test]
    fn test_extract_attachments() {
        use crate::db::utils::generate_entity_id;

        let skill_id = generate_entity_id();

        // Create test attachments
        let script_content = "#!/bin/bash\necho 'Hello'";
        let reference_content = "# Documentation\n\nThis is a reference.";

        let attachments = vec![
            SkillAttachment {
                id: generate_entity_id(),
                skill_id: skill_id.clone(),
                type_: "script".to_string(),
                filename: "test.sh".to_string(),
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
                filename: "README.md".to_string(),
                content: BASE64.encode(reference_content),
                content_hash: "def456".to_string(),
                mime_type: Some("text/markdown".to_string()),
                created_at: None,
                updated_at: None,
            },
        ];

        // Extract attachments
        let cache_dir = extract_attachments(&skill_id, &attachments).unwrap();

        // Verify directory structure
        assert!(cache_dir.join("scripts").exists());
        assert!(cache_dir.join("references").exists());
        assert!(cache_dir.join("assets").exists());

        // Verify files exist
        assert!(cache_dir.join("scripts/test.sh").exists());
        assert!(cache_dir.join("references/README.md").exists());

        // Verify content
        let script_read = fs::read_to_string(cache_dir.join("scripts/test.sh")).unwrap();
        assert_eq!(script_read, script_content);

        let reference_read = fs::read_to_string(cache_dir.join("references/README.md")).unwrap();
        assert_eq!(reference_read, reference_content);

        // Verify executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script_meta = fs::metadata(cache_dir.join("scripts/test.sh")).unwrap();
            let perms = script_meta.permissions();
            assert_eq!(perms.mode() & 0o111, 0o111); // Check executable bits
        }

        // Cleanup
        invalidate_cache(&skill_id).unwrap();
        assert!(!cache_dir.exists());
    }

    #[test]
    fn test_invalidate_cache() {
        use crate::db::utils::generate_entity_id;

        let skill_id = generate_entity_id();
        let cache_dir = get_skill_cache_dir(&skill_id);

        // Create cache directory
        fs::create_dir_all(&cache_dir).unwrap();
        assert!(cache_dir.exists());

        // Invalidate
        invalidate_cache(&skill_id).unwrap();
        assert!(!cache_dir.exists());

        // Invalidating non-existent cache should not error
        invalidate_cache(&skill_id).unwrap();
    }
}
