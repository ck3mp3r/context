//! Export database entities to JSONL files.

use crate::db::{
    Database, NoteRepository, ProjectRepository, RepoRepository, TaskListRepository, TaskRepository,
};
use std::path::Path;
use thiserror::Error;

use super::jsonl::{JsonlError, write_jsonl};

/// Errors that can occur during export.
#[derive(Error, Debug)]
pub enum ExportError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("JSONL error: {0}")]
    Jsonl(#[from] JsonlError),
}

/// Export all database entities to JSONL files in the specified directory.
///
/// Creates 5 files:
/// - repos.jsonl
/// - projects.jsonl
/// - lists.jsonl
/// - tasks.jsonl
/// - notes.jsonl
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
    let mut summary = ExportSummary::default();

    // Export repos
    let repos = db.repos().list(None).await?;
    write_jsonl(&output_dir.join("repos.jsonl"), &repos.items)?;
    summary.repos = repos.items.len();

    // Export projects
    let projects = db.projects().list(None).await?;
    write_jsonl(&output_dir.join("projects.jsonl"), &projects.items)?;
    summary.projects = projects.items.len();

    // Export task lists
    let task_lists = db.task_lists().list(None).await?;
    write_jsonl(&output_dir.join("lists.jsonl"), &task_lists.items)?;
    summary.task_lists = task_lists.items.len();

    // Export tasks
    let tasks = db.tasks().list(None).await?;
    write_jsonl(&output_dir.join("tasks.jsonl"), &tasks.items)?;
    summary.tasks = tasks.items.len();

    // Export notes
    let notes = db.notes().list(None).await?;
    write_jsonl(&output_dir.join("notes.jsonl"), &notes.items)?;
    summary.notes = notes.items.len();

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
}

impl ExportSummary {
    pub fn total(&self) -> usize {
        self.repos + self.projects + self.task_lists + self.tasks + self.notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Project, Repo, SqliteDatabase};
    use crate::sync::jsonl::read_jsonl;
    use tempfile::TempDir;

    async fn setup_test_db() -> SqliteDatabase {
        let db = SqliteDatabase::in_memory().await.unwrap();
        db.migrate().unwrap();
        db
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_export_empty_database() {
        let db = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        let summary = export_all(&db, temp_dir.path()).await.unwrap();

        // Note: migrations create a default project, so projects will be 1
        assert_eq!(summary.repos, 0);
        assert_eq!(summary.projects, 1); // Default project from migrations
        assert_eq!(summary.task_lists, 0);
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.notes, 0);

        // Verify files exist
        assert!(temp_dir.path().join("repos.jsonl").exists());
        assert!(temp_dir.path().join("projects.jsonl").exists());
        assert!(temp_dir.path().join("lists.jsonl").exists());
        assert!(temp_dir.path().join("tasks.jsonl").exists());
        assert!(temp_dir.path().join("notes.jsonl").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_export_with_data() {
        let db = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        // Create test data
        let repo = Repo {
            id: "12345678".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: Some("/test/path".to_string()),
            tags: vec!["test".to_string()],
            project_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        db.repos().create(&repo).await.unwrap();

        let project = Project {
            id: "abcdef12".to_string(),
            title: "Test Project".to_string(),
            description: Some("A test".to_string()),
            tags: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        db.projects().create(&project).await.unwrap();

        // Export
        let summary = export_all(&db, temp_dir.path()).await.unwrap();

        assert_eq!(summary.repos, 1);
        assert_eq!(summary.projects, 2); // 1 default + 1 we created
        assert_eq!(summary.total(), 3);

        // Verify JSONL content
        let repos: Vec<Repo> = read_jsonl(&temp_dir.path().join("repos.jsonl")).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "12345678");

        let projects: Vec<Project> = read_jsonl(&temp_dir.path().join("projects.jsonl")).unwrap();
        assert_eq!(projects.len(), 2); // default + our test project

        // Find our test project
        let our_project = projects.iter().find(|p| p.id == "abcdef12").unwrap();
        assert_eq!(our_project.title, "Test Project");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_export_creates_all_files() {
        let db = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        export_all(&db, temp_dir.path()).await.unwrap();

        // All 5 files should exist
        let expected_files = [
            "repos.jsonl",
            "projects.jsonl",
            "lists.jsonl",
            "tasks.jsonl",
            "notes.jsonl",
        ];

        for file in &expected_files {
            assert!(
                temp_dir.path().join(file).exists(),
                "File {} should exist",
                file
            );
        }
    }
}
