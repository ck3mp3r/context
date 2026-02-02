//! Import JSONL files into database.

use crate::db::{
    Database, Note, NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill,
    SkillAttachment, SkillRepository, Task, TaskList, TaskListRepository, TaskRepository,
};
use miette::Diagnostic;
use std::path::Path;
use thiserror::Error;

use super::jsonl::{JsonlError, read_jsonl};

/// Errors that can occur during import.
#[derive(Error, Diagnostic, Debug)]
pub enum ImportError {
    #[error("Database error: {0}")]
    #[diagnostic(code(c5t::sync::import::database))]
    Database(#[from] crate::db::DbError),

    #[error("JSONL error: {0}")]
    #[diagnostic(code(c5t::sync::import::jsonl))]
    Jsonl(#[from] JsonlError),

    #[error("File not found: {0}")]
    #[diagnostic(code(c5t::sync::import::file_not_found))]
    FileNotFound(String),
}

/// Import all JSONL files from the specified directory into the database.
///
/// Reads 6 files:
/// - repos.jsonl
/// - projects.jsonl
/// - lists.jsonl
/// - tasks.jsonl
/// - notes.jsonl
/// - skills.jsonl
///
/// Uses upsert logic: if entity exists (by ID), update it; otherwise create it.
///
/// # Arguments
/// * `db` - Database instance
/// * `input_dir` - Directory containing JSONL files
///
/// # Returns
/// A summary of imported entities (counts per type)
pub async fn import_all<D: Database>(
    db: &D,
    input_dir: &Path,
) -> Result<ImportSummary, ImportError> {
    tracing::debug!("Importing all entities from {:?}", input_dir);
    let mut summary = ImportSummary::default();

    // Import order respects foreign key dependencies:
    // 1. Projects (no FK dependencies)
    // 2. Repos (can reference projects)
    // 3. Task Lists (references projects)
    // 4. Tasks (references task_lists)
    // 5. Notes (can reference projects and repos)
    // 6. Skills (can reference projects)

    // Import projects FIRST (no dependencies)
    let projects_file = input_dir.join("projects.jsonl");
    if projects_file.exists() {
        tracing::debug!("Importing projects");
        let projects: Vec<Project> = read_jsonl(&projects_file)?;
        for project in projects {
            match db.projects().get(&project.id).await {
                Ok(_existing) => {
                    db.projects().update(&project).await?;
                }
                Err(_) => {
                    db.projects().create(&project).await?;
                }
            }
            summary.projects += 1;
        }
        tracing::debug!(count = summary.projects, "Imported projects");
    }

    // Import repos SECOND (can reference projects)
    let repos_file = input_dir.join("repos.jsonl");
    if repos_file.exists() {
        tracing::debug!("Importing repos");
        let repos: Vec<Repo> = read_jsonl(&repos_file)?;
        for repo in repos {
            match db.repos().get(&repo.id).await {
                Ok(_existing) => {
                    db.repos().update(&repo).await?;
                }
                Err(_) => {
                    db.repos().create(&repo).await?;
                }
            }
            summary.repos += 1;
        }
        tracing::debug!(count = summary.repos, "Imported repos");
    }

    // Import task lists
    let lists_file = input_dir.join("lists.jsonl");
    if lists_file.exists() {
        tracing::debug!("Importing task lists");
        let task_lists: Vec<TaskList> = read_jsonl(&lists_file)?;
        for task_list in task_lists {
            match db.task_lists().get(&task_list.id).await {
                Ok(_existing) => {
                    db.task_lists().update(&task_list).await?;
                }
                Err(_) => {
                    db.task_lists().create(&task_list).await?;
                }
            }
            summary.task_lists += 1;
        }
        tracing::debug!(count = summary.task_lists, "Imported task lists");
    }

    // Import tasks
    let tasks_file = input_dir.join("tasks.jsonl");
    if tasks_file.exists() {
        tracing::debug!("Importing tasks");
        let tasks: Vec<Task> = read_jsonl(&tasks_file)?;
        for task in tasks {
            match db.tasks().get(&task.id).await {
                Ok(_existing) => {
                    db.tasks().update(&task).await?;
                }
                Err(_) => {
                    db.tasks().create(&task).await?;
                }
            }
            summary.tasks += 1;
        }
        tracing::debug!(count = summary.tasks, "Imported tasks");
    }

    // Import notes
    let notes_file = input_dir.join("notes.jsonl");
    if notes_file.exists() {
        tracing::debug!("Importing notes");
        let notes: Vec<Note> = read_jsonl(&notes_file)?;
        for note in notes {
            match db.notes().get(&note.id).await {
                Ok(_existing) => {
                    db.notes().update(&note).await?;
                }
                Err(_) => {
                    db.notes().create(&note).await?;
                }
            }
            summary.notes += 1;
        }
        tracing::debug!(count = summary.notes, "Imported notes");
    }

    // Import skills
    let skills_file = input_dir.join("skills.jsonl");
    if skills_file.exists() {
        tracing::debug!("Importing skills");
        let skills: Vec<Skill> = read_jsonl(&skills_file)?;
        for skill in skills {
            // Upsert skill
            match db.skills().get(&skill.id).await {
                Ok(_existing) => {
                    db.skills().update(&skill).await?;
                }
                Err(_) => {
                    db.skills().create(&skill).await?;
                }
            }
            summary.skills += 1;
        }
        tracing::debug!(count = summary.skills, "Imported skills");
    }

    // Import skill attachments
    let attachments_file = input_dir.join("skills_attachments.jsonl");
    if attachments_file.exists() {
        tracing::debug!("Importing skill attachments");
        let attachments: Vec<SkillAttachment> = read_jsonl(&attachments_file)?;

        // Group attachments by skill_id for efficient processing
        let mut attachments_by_skill: std::collections::HashMap<String, Vec<SkillAttachment>> =
            std::collections::HashMap::new();
        for attachment in attachments {
            attachments_by_skill
                .entry(attachment.skill_id.clone())
                .or_default()
                .push(attachment);
        }

        // Process each skill's attachments
        for (skill_id, skill_attachments) in attachments_by_skill {
            // Get existing attachments for this skill
            let existing_attachments = db.skills().get_attachments(&skill_id).await?;

            // Upsert attachments - compare by skill_id + type + filename
            for attachment in &skill_attachments {
                let existing = existing_attachments.iter().find(|a| {
                    a.skill_id == attachment.skill_id
                        && a.type_ == attachment.type_
                        && a.filename == attachment.filename
                });

                match existing {
                    Some(existing_att) if existing_att.content_hash != attachment.content_hash => {
                        // Content changed - update attachment
                        tracing::debug!(
                            skill_id = %attachment.skill_id,
                            filename = %attachment.filename,
                            "Updating attachment (content changed)"
                        );
                        db.skills().update_attachment(attachment).await?;

                        // Invalidate cache since content changed
                        crate::skills::invalidate_cache(&skill_id)?;
                    }
                    Some(_) => {
                        // Content unchanged - skip
                        tracing::debug!(
                            skill_id = %attachment.skill_id,
                            filename = %attachment.filename,
                            "Skipping attachment (unchanged)"
                        );
                    }
                    None => {
                        // New attachment - create
                        tracing::debug!(
                            skill_id = %attachment.skill_id,
                            filename = %attachment.filename,
                            "Creating new attachment"
                        );
                        db.skills().create_attachment(attachment).await?;

                        // Invalidate cache to include new attachment
                        crate::skills::invalidate_cache(&skill_id)?;
                    }
                }
            }

            // Delete attachments that exist in DB but not in import
            for existing_att in existing_attachments {
                let in_import = skill_attachments.iter().any(|a| {
                    a.skill_id == existing_att.skill_id
                        && a.type_ == existing_att.type_
                        && a.filename == existing_att.filename
                });

                if !in_import {
                    tracing::debug!(
                        skill_id = %existing_att.skill_id,
                        filename = %existing_att.filename,
                        "Deleting attachment (not in import)"
                    );
                    db.skills().delete_attachment(&existing_att.id).await?;

                    // Invalidate cache to remove deleted attachment
                    crate::skills::invalidate_cache(&skill_id)?;
                }
            }

            summary.attachments += skill_attachments.len();
        }
        tracing::debug!(count = summary.attachments, "Imported skill attachments");
    }

    tracing::info!(total = summary.total(), "Import all complete");
    Ok(summary)
}

/// Summary of imported entities.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub repos: usize,
    pub projects: usize,
    pub task_lists: usize,
    pub tasks: usize,
    pub notes: usize,
    pub skills: usize,
    pub attachments: usize,
}

impl ImportSummary {
    pub fn total(&self) -> usize {
        self.repos
            + self.projects
            + self.task_lists
            + self.tasks
            + self.notes
            + self.skills
            + self.attachments
    }
}
