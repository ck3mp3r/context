//! Export database entities to JSONL files.

use crate::db::{
    Database, NoteRepository, ProjectRepository, RepoRepository, SkillRepository,
    TaskListRepository, TaskRepository,
};
use miette::Diagnostic;
use std::path::Path;
use thiserror::Error;

use super::jsonl::{JsonlError, write_jsonl};

/// Errors that can occur during export.
#[derive(Error, Diagnostic, Debug)]
pub enum ExportError {
    #[error("Database error: {0}")]
    #[diagnostic(code(c5t::sync::export::database))]
    Database(#[from] crate::db::DbError),

    #[error("JSONL error: {0}")]
    #[diagnostic(code(c5t::sync::export::jsonl))]
    Jsonl(#[from] JsonlError),
}

/// Export all database entities to JSONL files in the specified directory.
///
/// Creates 7 files:
/// - repos.jsonl
/// - projects.jsonl
/// - lists.jsonl
/// - tasks.jsonl
/// - notes.jsonl
/// - skills.jsonl
/// - skills_attachments.jsonl
///
/// # Arguments
/// * `db` - Database instance
/// * `output_dir` - Directory to write JSONL files to
///
/// # Returns
/// A summary of exported entities (counts per type)
pub async fn export_all<D: Database>(
    db: &D,
    output_dir: &Path,
) -> Result<ExportSummary, ExportError> {
    tracing::debug!("Exporting all entities to {:?}", output_dir);
    let mut summary = ExportSummary::default();

    // Export repos - get full entities with relationships
    tracing::debug!("Fetching repos");
    let repos_list = db.repos().list(None).await?;
    let mut repos = Vec::new();
    for repo in repos_list.items {
        let full_repo = db.repos().get(&repo.id).await?;
        repos.push(full_repo);
    }
    write_jsonl(&output_dir.join("repos.jsonl"), &repos)?;
    summary.repos = repos.len();
    tracing::debug!(count = repos.len(), "Exported repos");

    // Export projects - get full entities with relationships
    tracing::debug!("Fetching projects");
    let projects_list = db.projects().list(None).await?;
    let mut projects = Vec::new();
    for project in projects_list.items {
        let full_project = db.projects().get(&project.id).await?;
        projects.push(full_project);
    }
    write_jsonl(&output_dir.join("projects.jsonl"), &projects)?;
    summary.projects = projects.len();
    tracing::debug!(count = projects.len(), "Exported projects");

    // Export task lists - get full entities with relationships
    tracing::debug!("Fetching task lists");
    let task_lists_list = db.task_lists().list(None).await?;
    let mut task_lists = Vec::new();
    for task_list in task_lists_list.items {
        let full_task_list = db.task_lists().get(&task_list.id).await?;
        task_lists.push(full_task_list);
    }
    write_jsonl(&output_dir.join("lists.jsonl"), &task_lists)?;
    summary.task_lists = task_lists.len();
    tracing::debug!(count = task_lists.len(), "Exported task lists");

    // Export tasks (no relationships to fetch)
    tracing::debug!("Fetching tasks");
    let tasks = db.tasks().list(None).await?;
    write_jsonl(&output_dir.join("tasks.jsonl"), &tasks.items)?;
    summary.tasks = tasks.items.len();
    tracing::debug!(count = tasks.items.len(), "Exported tasks");

    // Export notes - get full entities with relationships
    tracing::debug!("Fetching notes");
    let notes_list = db.notes().list(None).await?;
    let mut notes = Vec::new();
    for note in notes_list.items {
        let full_note = db.notes().get(&note.id).await?;
        notes.push(full_note);
    }
    write_jsonl(&output_dir.join("notes.jsonl"), &notes)?;
    summary.notes = notes.len();
    tracing::debug!(count = notes.len(), "Exported notes");

    // Export skills with attachment filenames (computed fields)
    tracing::debug!("Fetching skills");
    let skills_list = db.skills().list(None).await?;
    let mut skills = Vec::new();
    let mut all_attachments = Vec::new();
    for skill in skills_list.items {
        let full_skill = db.skills().get(&skill.id).await?;
        let attachments = db.skills().get_attachments(&full_skill.id).await?;
        skills.push(full_skill);
        all_attachments.extend(attachments);
    }
    write_jsonl(&output_dir.join("skills.jsonl"), &skills)?;
    summary.skills = skills.len();
    tracing::debug!(count = skills.len(), "Exported skills");

    // Export skill attachments - one attachment per line
    let attachments_path = output_dir.join("skills_attachments.jsonl");
    tracing::warn!(
        "ABOUT TO WRITE {} attachments to {:?}",
        all_attachments.len(),
        attachments_path
    );
    write_jsonl(&attachments_path, &all_attachments)?;
    tracing::warn!(
        "WROTE {} attachments to {:?}",
        all_attachments.len(),
        attachments_path
    );
    summary.attachments = all_attachments.len();
    tracing::debug!(count = all_attachments.len(), "Exported skill attachments");

    tracing::info!(total = summary.total(), "Export all complete");
    Ok(summary)
}

/// Summary of exported entities.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub repos: usize,
    pub projects: usize,
    pub task_lists: usize,
    pub tasks: usize,
    pub notes: usize,
    pub skills: usize,
    pub attachments: usize,
}

impl ExportSummary {
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
