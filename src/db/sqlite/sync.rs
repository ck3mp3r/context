//! SQLite-specific sync repository implementation.

use sqlx::SqlitePool;
use std::path::Path;

use crate::db::{DbError, DbResult, Note, Project, Repo, SyncRepository, Task, TaskList};
use crate::sync::{ExportSummary, ImportSummary, read_jsonl};

/// SQLite-specific sync repository.
pub struct SqliteSyncRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> SyncRepository for SqliteSyncRepository<'a> {
    async fn import_all(&self, input_dir: &Path) -> DbResult<ImportSummary> {
        // Begin transaction
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: format!("Failed to begin transaction: {}", e),
        })?;

        // Enable deferred FK constraints for this transaction ONLY
        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Failed to set PRAGMA defer_foreign_keys: {}", e),
            })?;

        // Perform import using transaction
        let summary = import_all_with_transaction(&mut tx, input_dir)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Import failed: {}", e),
            })?;

        // Commit transaction (FK constraints validated here)
        tx.commit().await.map_err(|e| {
            if e.to_string().contains("FOREIGN KEY constraint failed")
                || e.to_string().contains("foreign key")
            {
                DbError::Constraint {
                    message: format!(
                        "Foreign key constraint violation during import. \
                         Referenced entity doesn't exist: {}",
                        e
                    ),
                }
            } else {
                DbError::Database {
                    message: format!("Failed to commit: {}", e),
                }
            }
        })?;

        Ok(summary)
    }

    async fn export_all(&self, output_dir: &Path) -> DbResult<ExportSummary> {
        export_all_from_pool(self.pool, output_dir)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Export failed: {}", e),
            })
    }
}

/// Import all JSONL files using a provided SQLite transaction.
///
/// This is SQLite-specific because it uses raw SQL queries within a transaction.
async fn import_all_with_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    input_dir: &Path,
) -> Result<ImportSummary, Box<dyn std::error::Error + Send + Sync>> {
    let mut summary = ImportSummary::default();

    // Import order (with deferred FK, this doesn't matter, but keep logical):
    // 1. Projects (no FK dependencies)
    // 2. Repos (can reference projects via project_repo M:N)
    // 3. Task Lists (references projects)
    // 4. Tasks (references task_lists and optionally parent tasks)
    // 5. Notes (can reference projects and repos)

    // ========== Import Projects ==========
    let projects_file = input_dir.join("projects.jsonl");
    if projects_file.exists() {
        let projects: Vec<Project> = read_jsonl(&projects_file)?;
        for project in projects {
            // Upsert project
            sqlx::query(
                "INSERT INTO project (id, title, description, tags, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title,
                   description = excluded.description,
                   tags = excluded.tags,
                   updated_at = excluded.updated_at",
            )
            .bind(&project.id)
            .bind(&project.title)
            .bind(&project.description)
            .bind(serde_json::to_string(&project.tags)?)
            .bind(&project.created_at)
            .bind(&project.updated_at)
            .execute(&mut **tx)
            .await?;

            summary.projects += 1;
        }
    }

    // ========== Import Repos ==========
    let repos_file = input_dir.join("repos.jsonl");
    if repos_file.exists() {
        let repos: Vec<Repo> = read_jsonl(&repos_file)?;
        for repo in repos {
            // Upsert repo
            sqlx::query(
                "INSERT INTO repo (id, remote, path, tags, created_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   remote = excluded.remote,
                   path = excluded.path,
                   tags = excluded.tags",
            )
            .bind(&repo.id)
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(serde_json::to_string(&repo.tags)?)
            .bind(&repo.created_at)
            .execute(&mut **tx)
            .await?;

            // Handle project_repo M:N relationships
            // Delete existing relationships for this repo
            sqlx::query("DELETE FROM project_repo WHERE repo_id = ?")
                .bind(&repo.id)
                .execute(&mut **tx)
                .await?;

            // Insert new relationships
            for project_id in &repo.project_ids {
                sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
                    .bind(project_id)
                    .bind(&repo.id)
                    .execute(&mut **tx)
                    .await?;
            }

            summary.repos += 1;
        }
    }

    // ========== Import Task Lists ==========
    let lists_file = input_dir.join("lists.jsonl");
    if lists_file.exists() {
        let task_lists: Vec<TaskList> = read_jsonl(&lists_file)?;
        for task_list in task_lists {
            // Upsert task_list
            sqlx::query(
                "INSERT INTO task_list (id, title, description, notes, project_id, tags, status, external_ref, created_at, updated_at, archived_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title,
                   description = excluded.description,
                   notes = excluded.notes,
                   project_id = excluded.project_id,
                   tags = excluded.tags,
                   status = excluded.status,
                   external_ref = excluded.external_ref,
                   updated_at = excluded.updated_at,
                   archived_at = excluded.archived_at",
            )
            .bind(&task_list.id)
            .bind(&task_list.title)
            .bind(&task_list.description)
            .bind(&task_list.notes)
            .bind(&task_list.project_id)
            .bind(serde_json::to_string(&task_list.tags)?)
            .bind(task_list.status.to_string())
            .bind(&task_list.external_ref)
            .bind(&task_list.created_at)
            .bind(&task_list.updated_at)
            .bind(&task_list.archived_at)
            .execute(&mut **tx)
            .await?;

            // Handle task_list_repo M:N relationships
            sqlx::query("DELETE FROM task_list_repo WHERE task_list_id = ?")
                .bind(&task_list.id)
                .execute(&mut **tx)
                .await?;

            for repo_id in &task_list.repo_ids {
                sqlx::query("INSERT INTO task_list_repo (task_list_id, repo_id) VALUES (?, ?)")
                    .bind(&task_list.id)
                    .bind(repo_id)
                    .execute(&mut **tx)
                    .await?;
            }

            summary.task_lists += 1;
        }
    }

    // ========== Import Tasks ==========
    let tasks_file = input_dir.join("tasks.jsonl");
    if tasks_file.exists() {
        let tasks: Vec<Task> = read_jsonl(&tasks_file)?;
        for task in tasks {
            // Upsert task
            sqlx::query(
                "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, started_at, completed_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   list_id = excluded.list_id,
                   parent_id = excluded.parent_id,
                   title = excluded.title,
                   description = excluded.description,
                   status = excluded.status,
                   priority = excluded.priority,
                   tags = excluded.tags,
                   started_at = excluded.started_at,
                   completed_at = excluded.completed_at,
                   updated_at = excluded.updated_at",
            )
            .bind(&task.id)
            .bind(&task.list_id)
            .bind(&task.parent_id)
            .bind(&task.title)
            .bind(&task.description)
            .bind(task.status.to_string())
            .bind(task.priority)
            .bind(serde_json::to_string(&task.tags)?)
            .bind(&task.created_at)
            .bind(&task.started_at)
            .bind(&task.completed_at)
            .bind(&task.updated_at)
            .execute(&mut **tx)
            .await?;

            summary.tasks += 1;
        }
    }

    // ========== Import Notes ==========
    let notes_file = input_dir.join("notes.jsonl");
    if notes_file.exists() {
        let notes: Vec<Note> = read_jsonl(&notes_file)?;
        for note in notes {
            // Upsert note
            sqlx::query(
                "INSERT INTO note (id, title, content, tags, parent_id, idx, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   title = excluded.title,
                   content = excluded.content,
                   tags = excluded.tags,
                   parent_id = excluded.parent_id,
                   idx = excluded.idx,
                   updated_at = excluded.updated_at",
            )
            .bind(&note.id)
            .bind(&note.title)
            .bind(&note.content)
            .bind(serde_json::to_string(&note.tags)?)
            .bind(&note.parent_id)
            .bind(&note.idx)
            .bind(&note.created_at)
            .bind(&note.updated_at)
            .execute(&mut **tx)
            .await?;

            // Handle project_note M:N relationships
            sqlx::query("DELETE FROM project_note WHERE note_id = ?")
                .bind(&note.id)
                .execute(&mut **tx)
                .await?;

            for project_id in &note.project_ids {
                sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
                    .bind(project_id)
                    .bind(&note.id)
                    .execute(&mut **tx)
                    .await?;
            }

            // Handle note_repo M:N relationships
            sqlx::query("DELETE FROM note_repo WHERE note_id = ?")
                .bind(&note.id)
                .execute(&mut **tx)
                .await?;

            for repo_id in &note.repo_ids {
                sqlx::query("INSERT INTO note_repo (note_id, repo_id) VALUES (?, ?)")
                    .bind(&note.id)
                    .bind(repo_id)
                    .execute(&mut **tx)
                    .await?;
            }

            summary.notes += 1;
        }
    }

    Ok(summary)
}

/// Export all database entities to JSONL files using a SQLite pool.
///
/// Uses the repository pattern through a temporary SqliteDatabase instance.
async fn export_all_from_pool(
    pool: &SqlitePool,
    output_dir: &Path,
) -> Result<ExportSummary, Box<dyn std::error::Error + Send + Sync>> {
    use crate::db::sqlite::{
        SqliteNoteRepository, SqliteProjectRepository, SqliteRepoRepository,
        SqliteTaskListRepository, SqliteTaskRepository,
    };
    use crate::db::{
        NoteRepository, ProjectRepository, RepoRepository, TaskListRepository, TaskRepository,
    };
    use crate::sync::write_jsonl;

    let mut summary = ExportSummary::default();

    // Export repos - get full entities with relationships
    let repos_repo = SqliteRepoRepository { pool };
    let repos_list = repos_repo.list(None).await?;
    let mut repos = Vec::new();
    for repo in repos_list.items {
        let full_repo = repos_repo.get(&repo.id).await?;
        repos.push(full_repo);
    }
    write_jsonl(&output_dir.join("repos.jsonl"), &repos)?;
    summary.repos = repos.len();

    // Export projects - get full entities with relationships
    let projects_repo = SqliteProjectRepository { pool };
    let projects_list = projects_repo.list(None).await?;
    let mut projects = Vec::new();
    for project in projects_list.items {
        let full_project = projects_repo.get(&project.id).await?;
        projects.push(full_project);
    }
    write_jsonl(&output_dir.join("projects.jsonl"), &projects)?;
    summary.projects = projects.len();

    // Export task lists - get full entities with relationships
    let task_lists_repo = SqliteTaskListRepository { pool };
    let task_lists_list = task_lists_repo.list(None).await?;
    let mut task_lists = Vec::new();
    for task_list in task_lists_list.items {
        let full_task_list = task_lists_repo.get(&task_list.id).await?;
        task_lists.push(full_task_list);
    }
    write_jsonl(&output_dir.join("lists.jsonl"), &task_lists)?;
    summary.task_lists = task_lists.len();

    // Export tasks (no relationships to fetch)
    let tasks_repo = SqliteTaskRepository { pool };
    let tasks = tasks_repo.list(None).await?;
    write_jsonl(&output_dir.join("tasks.jsonl"), &tasks.items)?;
    summary.tasks = tasks.items.len();

    // Export notes - get full entities with relationships
    let notes_repo = SqliteNoteRepository { pool };
    let notes_list = notes_repo.list(None).await?;
    let mut notes = Vec::new();
    for note in notes_list.items {
        let full_note = notes_repo.get(&note.id).await?;
        notes.push(full_note);
    }
    write_jsonl(&output_dir.join("notes.jsonl"), &notes)?;
    summary.notes = notes.len();

    Ok(summary)
}
