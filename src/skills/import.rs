//! Skill import orchestration
//!
//! Coordinates the import process:
//! 1. Parse source URL
//! 2. Fetch source to temp directory
//! 3. Parse SKILL.md
//! 4. Scan attachments
//! 5. Insert into database
//! 6. Cleanup temp files

use crate::db::{Database, Skill};
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
/// * `source` - Source URL/path (git+https://, git+ssh://, file://, https://.zip, etc.)
/// * `subpath` - Optional subpath override within the source
/// * `project_ids` - Optional list of project IDs to link
///
/// # Returns
/// The created skill with generated ID
pub async fn import_skill<D: Database>(
    _db: &D,
    _source: &str,
    _subpath: Option<&str>,
    _project_ids: Option<Vec<String>>,
) -> Result<Skill, ImportError> {
    // TODO: Implement import orchestration
    // 1. Parse source using source::parse_source()
    // 2. Fetch source using source::fetch_source()
    // 3. Navigate to subpath if specified
    // 4. Check for SKILL.md existence
    // 5. Parse SKILL.md using parser::parse_skill_md()
    // 6. Scan attachments using scanner::scan_attachments()
    // 7. Generate skill ID
    // 8. Create skill in database
    // 9. Create attachments in database (via SqliteSkillRepository)
    // 10. Link to projects if specified
    // 11. Cleanup temp directory
    // 12. Return created skill

    Err(ImportError::ImportFailed(
        "Import not yet implemented".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SqliteDatabase;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_placeholder() {
        let db = SqliteDatabase::in_memory()
            .await
            .expect("Failed to create in-memory database");
        db.migrate().expect("Migration should succeed");

        let result = import_skill(&db, "git+https://github.com/user/repo/skill", None, None).await;

        assert!(result.is_err()); // Placeholder until implemented
    }
}
