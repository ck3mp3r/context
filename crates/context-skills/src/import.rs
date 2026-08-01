//! Skill import orchestration
//!
//! Coordinates the import process:
//! 1. Parse source URL
//! 2. Fetch source to temp directory
//! 3. Parse SKILL.md
//! 4. Scan attachments
//! 5. Insert into database
//! 6. Cleanup temp files

use context_core::{HasProjects, HasSkills, Skill, SkillAttachment, SkillRepository};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("Source error: {0}")]
    Source(#[from] crate::source::SourceError),

    #[error("Parser error: {0}")]
    Parser(#[from] crate::parser::ParserError),

    #[error("Scanner error: {0}")]
    Scanner(#[from] crate::scanner::ScannerError),

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
/// * `tags` - Optional list of tags to apply
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
///     Some(vec!["kubernetes".to_string(), "deployment".to_string()]),
///     false
/// ).await?;
///
/// // Import or update skill
/// let skill = import_skill(
///     &db,
///     "git+https://github.com/agentskills/deploy-k8s",
///     Some("skills/deploy"),
///     Some(vec!["project123".to_string()]),
///     Some(vec!["kubernetes".to_string()]),
///     true
/// ).await?;
/// ```
pub async fn import_skill<D: HasSkills + HasProjects>(
    db: &D,
    source: &str,
    subpath: Option<&str>,
    project_ids: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    upsert: bool,
) -> Result<Skill, ImportError> {
    // Parse source URL to determine type (git+https, git+ssh, local path)
    let source_type = crate::source::parse_source(source)?;

    // Fetch source to a directory (clone for git, validate for local)
    let source_path = crate::source::fetch_source(source_type)?;

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
        let parsed = crate::parser::parse_skill_md(&skill_md_path)?;

        // Scan attachments
        let attachments = crate::scanner::scan_attachments(&skill_dir)?;

        // Generate deterministic skill ID from name (8-char hex checksum)
        // This ensures same skill name = same ID, preventing duplicates
        let skill_id = crate::generate_skill_id(&parsed.name);

        // Check if skill already exists (before creating the new skill struct)
        let existing = db.skills().get(&skill_id).await.ok();

        // Determine tags and project_ids: preserve existing if not provided, otherwise use new
        let final_tags = match (&tags, &existing) {
            (Some(new_tags), _) => new_tags.clone(), // Explicitly provided -> use new
            (None, Some(existing_skill)) => existing_skill.tags.clone(), // Not provided + exists -> preserve
            (None, None) => vec![], // Not provided + doesn't exist -> empty
        };

        let final_project_ids = match (&project_ids, &existing) {
            (Some(new_ids), _) => new_ids.clone(), // Explicitly provided -> use new
            (None, Some(existing_skill)) => existing_skill.project_ids.clone(), // Not provided + exists -> preserve
            (None, None) => vec![], // Not provided + doesn't exist -> empty
        };

        // Create skill
        let skill = Skill {
            id: skill_id.clone(),
            name: parsed.name,
            description: parsed.description,
            content: parsed.content,
            tags: final_tags,
            project_ids: final_project_ids,
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

        // Check if we need to update (skill exists and upsert is true)
        if let Some(_existing_skill) = existing {
            if !upsert {
                return Err(ImportError::ValidationError(format!(
                    "Skill '{}' (ID: {}) already exists. Use --update flag to update it.",
                    skill.name, skill_id
                )));
            }

            // Update existing skill (preserves created_at timestamp)
            db.skills()
                .update(&skill)
                .await
                .map_err(|e| ImportError::Database(e.to_string()))?;

            // Delete all old attachments for this skill in one query
            // Much more efficient than iterating and deleting one by one
            db.skills()
                .delete_attachments_for_skill(&skill_id)
                .await
                .map_err(|e| ImportError::Database(e.to_string()))?;
        } else {
            // Create new skill
            db.skills()
                .create(&skill)
                .await
                .map_err(|e| ImportError::Database(e.to_string()))?;
        }

        // Create (new) attachments - always runs for both create and update paths
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


