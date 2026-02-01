//! Skill import orchestration
//!
//! Coordinates the import process:
//! 1. Parse source URL
//! 2. Fetch source to temp directory
//! 3. Parse SKILL.md
//! 4. Scan attachments
//! 5. Insert into database
//! 6. Cleanup temp files

use crate::db::{Database, Skill, SkillAttachment, SkillRepository};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Source error: {0}")]
    Source(#[from] super::source::SourceError),

    #[error("Parser error: {0}")]
    Parser(#[from] super::parser::ParserError),

    #[error("Scanner error: {0}")]
    Scanner(#[from] super::scanner::ScannerError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Skill validation failed: {0}")]
    ValidationError(String),

    #[error("SKILL.md not found in source")]
    SkillMdNotFound,

    #[error("Import operation failed: {0}")]
    ImportFailed(String),
}

/// Import a skill from a source URL
///
/// # Arguments
/// * `db` - Database handle
/// * `source` - Source URL/path (git+https://, git+ssh://, file://, local path)
/// * `subpath` - Optional subpath within the source (e.g., "skills/deploy")
/// * `project_ids` - Optional list of project IDs to link
/// * `upsert` - If true, update existing skill if it exists; if false, fail on duplicate
///
/// # Returns
/// The created or updated skill with generated ID
///
/// # Supported Sources
/// - `git+https://github.com/user/repo` - Git clone via HTTPS
/// - `git+ssh://git@github.com/user/repo.git` - Git clone via SSH
/// - `file:///absolute/path` - Local filesystem (absolute)
/// - `/absolute/path` - Local filesystem (absolute)
/// - `./relative/path` - Local filesystem (relative)
///
/// # Example
/// ```ignore
/// // Import new skill (fails if exists)
/// let skill = import_skill(
///     &db,
///     "git+https://github.com/agentskills/deploy-k8s",
///     Some("skills/deploy"),
///     Some(vec!["project123".to_string()]),
///     false
/// ).await?;
///
/// // Import or update skill
/// let skill = import_skill(
///     &db,
///     "git+https://github.com/agentskills/deploy-k8s",
///     Some("skills/deploy"),
///     Some(vec!["project123".to_string()]),
///     true
/// ).await?;
/// ```
pub async fn import_skill<D: Database>(
    db: &D,
    source: &str,
    subpath: Option<&str>,
    project_ids: Option<Vec<String>>,
    upsert: bool,
) -> Result<Skill, ImportError> {
    // Parse source URL to determine type (git+https, git+ssh, local path)
    let source_type = super::source::parse_source(source)?;

    // Fetch source to a directory (clone for git, validate for local)
    let source_path = super::source::fetch_source(source_type)?;

    // Determine temp directory to clean up (only for Git clones)
    // Git clones create: /tmp/c5t-skill-import-{pid}
    // If skill_dir is inside this, we need to clean up the parent temp dir
    let temp_dir = std::env::temp_dir().join(format!("c5t-skill-import-{}", std::process::id()));
    let should_cleanup = source_path.starts_with(&temp_dir);

    // Navigate to subpath if specified
    let skill_dir = if let Some(path) = subpath {
        source_path.join(path)
    } else {
        source_path
    };

    // Import logic wrapped to ensure cleanup on success or failure
    let result = async {
        // Check for SKILL.md existence
        let skill_md_path = skill_dir.join("SKILL.md");
        if !skill_md_path.exists() {
            return Err(ImportError::SkillMdNotFound);
        }

        // Parse SKILL.md (extract name + description, store full content)
        let parsed = super::parser::parse_skill_md(&skill_md_path)?;

        // Scan attachments
        let attachments = super::scanner::scan_attachments(&skill_dir)?;

        // Generate deterministic skill ID from name (8-char hex checksum)
        // This ensures same skill name = same ID, preventing duplicates
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(parsed.name.as_bytes());
        let hash = hasher.finalize();
        let skill_id = format!(
            "{:x}",
            &hash[..4].iter().fold(0u32, |acc, &b| (acc << 8) | b as u32)
        );

        // Create skill
        let skill = Skill {
            id: skill_id.clone(),
            name: parsed.name,
            description: parsed.description,
            content: parsed.content,
            tags: vec![],
            project_ids: project_ids.unwrap_or_default(),
            scripts: attachments
                .iter()
                .filter(|a| a.type_ == "script")
                .map(|a| a.filename.clone())
                .collect(),
            references: attachments
                .iter()
                .filter(|a| a.type_ == "reference")
                .map(|a| a.filename.clone())
                .collect(),
            assets: attachments
                .iter()
                .filter(|a| a.type_ == "asset")
                .map(|a| a.filename.clone())
                .collect(),
            created_at: None,
            updated_at: None,
        };

        // Check if skill already exists
        let existing = db.skills().get(&skill_id).await.ok();

        if let Some(_existing_skill) = existing {
            if !upsert {
                return Err(ImportError::ValidationError(format!(
                    "Skill '{}' (ID: {}) already exists. Use --update flag to update it.",
                    skill.name, skill_id
                )));
            }

            // Delete existing skill (CASCADE will delete all attachments)
            db.skills()
                .delete(&skill_id)
                .await
                .map_err(|e| ImportError::Database(e.to_string()))?;
        }

        // Create (or re-create) skill
        db.skills()
            .create(&skill)
            .await
            .map_err(|e| ImportError::Database(e.to_string()))?;

        // Create attachments
        for attachment_data in attachments {
            let attachment = SkillAttachment {
                id: String::new(),
                skill_id: skill_id.clone(),
                type_: attachment_data.type_,
                filename: attachment_data.filename,
                content: attachment_data.content_base64,
                content_hash: attachment_data.content_hash,
                mime_type: attachment_data.mime_type,
                created_at: None,
                updated_at: None,
            };
            db.skills()
                .create_attachment(&attachment)
                .await
                .map_err(|e| ImportError::Database(e.to_string()))?;
        }

        Ok(skill)
    }
    .await;

    // Cleanup temp directory if this was a Git clone
    if should_cleanup && temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok(); // Ignore cleanup errors
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteDatabase;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_local_path() {
        let db = SqliteDatabase::in_memory()
            .await
            .expect("Failed to create in-memory database");
        db.migrate().expect("Migration should succeed");

        // Create a temporary skill directory
        let temp_dir = std::env::temp_dir().join(format!("test-skill-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create SKILL.md
        let skill_md = temp_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            r#"---
name: Test Skill
description: A test skill
---

# Test Skill

This is a test skill for import testing.
"#,
        )
        .unwrap();

        // Create a script
        std::fs::create_dir_all(temp_dir.join("scripts")).unwrap();
        std::fs::write(temp_dir.join("scripts/test.sh"), "#!/bin/bash\necho test").unwrap();

        // Import the skill (no upsert)
        let result = import_skill(&db, temp_dir.to_str().unwrap(), None, None, false).await;

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();

        assert!(result.is_ok(), "Import should succeed");
        let skill = result.unwrap();
        assert_eq!(skill.name, "Test Skill");
        assert_eq!(skill.description, "A test skill");
        assert_eq!(skill.scripts.len(), 1);
        assert_eq!(skill.scripts[0], "scripts/test.sh"); // Full relative path
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_duplicate_without_update_fails() {
        let db = SqliteDatabase::in_memory()
            .await
            .expect("Failed to create in-memory database");
        db.migrate().expect("Migration should succeed");

        // Create a temporary skill directory
        let temp_dir = std::env::temp_dir().join(format!("test-skill-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create SKILL.md
        let skill_md = temp_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            r#"---
name: Test Skill
description: A test skill
---

# Test Skill
"#,
        )
        .unwrap();

        // First import should succeed
        let result1 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, false).await;
        assert!(result1.is_ok(), "First import should succeed");

        // Second import without update flag should fail
        let result2 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, false).await;
        assert!(result2.is_err(), "Second import without update should fail");

        match result2.unwrap_err() {
            ImportError::ValidationError(msg) => {
                assert!(msg.contains("already exists"));
                assert!(msg.contains("--update"));
            }
            _ => panic!("Expected ValidationError"),
        }

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_duplicate_with_update_succeeds() {
        let db = SqliteDatabase::in_memory()
            .await
            .expect("Failed to create in-memory database");
        db.migrate().expect("Migration should succeed");

        // Create a temporary skill directory
        let temp_dir =
            std::env::temp_dir().join(format!("test-skill-update-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create SKILL.md
        let skill_md = temp_dir.join("SKILL.md");
        std::fs::write(
            &skill_md,
            r#"---
name: Test Update Skill
description: Original description
---

# Test Skill
"#,
        )
        .unwrap();

        // First import
        let result1 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, false).await;
        assert!(result1.is_ok(), "First import should succeed");
        let skill1 = result1.unwrap();
        assert_eq!(skill1.description, "Original description");

        // Update SKILL.md
        std::fs::write(
            &skill_md,
            r#"---
name: Test Update Skill
description: Updated description
---

# Test Skill Updated
"#,
        )
        .unwrap();

        // Second import with update flag should succeed
        let result2 = import_skill(&db, temp_dir.to_str().unwrap(), None, None, true).await;
        assert!(result2.is_ok(), "Second import with update should succeed");
        let skill2 = result2.unwrap();
        assert_eq!(skill2.id, skill1.id, "ID should be the same");
        assert_eq!(skill2.description, "Updated description");

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }
}
