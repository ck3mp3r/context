//! Cache management for skill attachments.
//!
//! Skills can have attachments stored as base64-encoded content in the database.
//! When a skill is loaded via `c5t_get_skill`, attachments are extracted to a local
//! cache directory using the skill name (per Agent Skills spec) and preserving the
//! exact directory structure from the source.
//!
//! Cache structure (example from docx skill):
//! ```text
//! ~/.local/share/c5t-dev/skills/docx/  (debug builds)
//! ~/.local/share/c5t/skills/docx/      (release builds)
//!   ├── SKILL.md              (from skill content field)
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
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::db::{DbError, SkillAttachment};
use crate::sync::get_data_dir;

/// Minimal frontmatter structure for extracting skill name
#[derive(Debug, Deserialize)]
struct MinimalFrontmatter {
    name: String,
}

/// Get the base cache directory for skills.
/// Returns: `~/.local/share/c5t-dev/skills/` (debug) or `~/.local/share/c5t/skills/` (release)
pub fn get_skills_cache_dir() -> PathBuf {
    get_data_dir().join("skills")
}

/// Get the cache directory for a specific skill.
/// Returns: `~/.local/share/c5t-dev/skills/<skill-name>/` (debug) or `~/.local/share/c5t/skills/<skill-name>/` (release)
///
/// # Arguments
/// * `skill_name` - Skill name from SKILL.md frontmatter
pub fn get_skill_cache_dir(skill_name: &str) -> PathBuf {
    get_skills_cache_dir().join(skill_name)
}

/// Extract skill attachments to cache.
///
/// Creates a cache directory using the skill name (per Agent Skills spec),
/// writes SKILL.md from content, and extracts all attachments preserving
/// directory structure.
///
/// For shell scripts (.sh, .bash), sets executable permissions.
///
/// # Arguments
/// * `skills_base_dir` - Base directory for skills cache (e.g., ~/.local/share/c5t/skills or ~/.agents/skills)
/// * `skill_name` - Skill name from frontmatter (used as cache directory name)
/// * `skill_content` - Full SKILL.md content to write to cache
/// * `attachments` - List of attachments to extract
///
/// # Returns
/// Path to skill cache directory
pub fn extract_attachments(
    skills_base_dir: &std::path::Path,
    skill_name: &str,
    skill_content: &str,
    attachments: &[SkillAttachment],
) -> Result<PathBuf, DbError> {
    let cache_dir = skills_base_dir.join(skill_name);

    // Create cache directory
    fs::create_dir_all(&cache_dir).map_err(|e| DbError::Database {
        message: format!(
            "Failed to create cache directory {}: {}",
            cache_dir.display(),
            e
        ),
    })?;

    // Write SKILL.md
    let skill_md_path = cache_dir.join("SKILL.md");
    fs::write(&skill_md_path, skill_content).map_err(|e| DbError::Database {
        message: format!(
            "Failed to write SKILL.md to {}: {}",
            skill_md_path.display(),
            e
        ),
    })?;

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
/// * `skill_name` - Skill name to invalidate cache for
pub fn invalidate_cache(skill_name: &str) -> Result<(), DbError> {
    let cache_dir = get_skill_cache_dir(skill_name);

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

/// Parse skill name from SKILL.md content.
///
/// Extracts the `name` field from YAML frontmatter. The name is used as the
/// cache directory name (per Agent Skills spec, directory name must match skill name).
///
/// # Arguments
/// * `content` - Full SKILL.md content with YAML frontmatter
///
/// # Returns
/// Skill name from frontmatter
///
/// # Errors
/// Returns error if:
/// - Content doesn't have valid YAML frontmatter (must start/end with ---)
/// - Frontmatter doesn't contain `name` field
/// - Name field is empty
pub fn parse_skill_name_from_content(content: &str) -> Result<String, DbError> {
    // Extract YAML frontmatter (content between --- delimiters)
    let frontmatter = extract_frontmatter(content)?;

    // Parse the YAML to get just the name field
    let minimal: MinimalFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(|e| DbError::Database {
            message: format!("Failed to parse YAML frontmatter: {}", e),
        })?;

    // Validate name is not empty
    if minimal.name.is_empty() {
        return Err(DbError::Database {
            message: "Skill name cannot be empty".to_string(),
        });
    }

    Ok(minimal.name)
}

/// Extract YAML frontmatter from SKILL.md content.
///
/// Expected format:
/// ```text
/// ---
/// name: skill-name
/// description: Description here
/// ---
/// # Instructions
/// ...
/// ```
fn extract_frontmatter(content: &str) -> Result<String, DbError> {
    let lines: Vec<&str> = content.lines().collect();

    // Check for opening ---
    if lines.is_empty() || lines[0].trim() != "---" {
        return Err(DbError::Database {
            message: "Invalid SKILL.md format: missing opening ---".to_string(),
        });
    }

    // Find closing ---
    let closing_index = lines
        .iter()
        .skip(1)
        .position(|line| line.trim() == "---")
        .ok_or_else(|| DbError::Database {
            message: "Invalid SKILL.md format: missing closing ---".to_string(),
        })?
        + 1; // +1 because we skipped the first line

    // Extract frontmatter (between the --- markers)
    let frontmatter = lines[1..closing_index].join("\n");

    Ok(frontmatter)
}
