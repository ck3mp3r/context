//! SQLite-specific sync repository implementation.

use futures_util::TryStreamExt;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use std::path::Path;

use context_core::{
    DbError, DbResult, ExportSummary, ImportSummary, JsonlReader, JsonlWriter, Note, Project, Repo,
    Skill, SyncRepository, Task, TaskList,
};

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

    async fn last_synced_at(&self) -> DbResult<Option<String>> {
        read_sync_cutoff(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Failed to read sync state: {}", e),
            })
    }

    async fn set_last_synced_at(&self, timestamp: &str) -> DbResult<()> {
        write_sync_cutoff(self.pool, timestamp)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Failed to write sync state: {}", e),
            })
    }
}

/// Maximum number of bind parameters per SQLite statement.
///
/// SQLite's default `SQLITE_MAX_VARIABLE_NUMBER` is 32766. Batched statements
/// stay below that limit so a single statement never exceeds it.
pub(crate) const MAX_BIND_PARAMS: usize = 32_000;

/// Number of rows that fit in one batched statement with `columns` binds per row.
pub(crate) fn rows_per_batch(columns: usize) -> usize {
    (MAX_BIND_PARAMS / columns).max(1)
}

/// Read up to `batch_size` entities from a streaming JSONL reader.
///
/// Returns an empty vector at end of file. Memory stays bounded by
/// `batch_size` regardless of the file size.
pub(crate) fn read_batch<T: for<'de> serde::Deserialize<'de>>(
    reader: &mut JsonlReader,
    batch_size: usize,
) -> Result<Vec<T>, context_core::JsonlError> {
    let mut batch = Vec::with_capacity(batch_size);
    while batch.len() < batch_size {
        match reader.next_entity::<T>()? {
            Some(entity) => batch.push(entity),
            None => break,
        }
    }
    Ok(batch)
}

/// Execute a batched upsert: one multi-row statement per chunk of `rows`.
///
/// `sql_prefix` is the `INSERT INTO ...` clause, `sql_suffix` is the
/// `ON CONFLICT ...` clause, and `bind_row` pushes one row's bind values.
async fn batched_upsert<T, F>(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    rows: &[T],
    columns: usize,
    sql_prefix: &str,
    sql_suffix: &str,
    mut bind_row: F,
) -> Result<(), sqlx::Error>
where
    F: for<'qb> FnMut(&mut sqlx::query_builder::Separated<'qb, Sqlite, &'static str>, &T),
{
    for chunk in rows.chunks(rows_per_batch(columns)) {
        let mut builder = QueryBuilder::<Sqlite>::new(sql_prefix);
        builder.push_values(chunk, |mut separated, row| bind_row(&mut separated, row));
        builder.push(sql_suffix);
        builder.build().execute(&mut **tx).await?;
    }
    Ok(())
}

/// Delete every relationship row whose `column` value is in `ids`.
async fn delete_relationships(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    table: &str,
    column: &str,
    ids: &[String],
) -> Result<(), sqlx::Error> {
    for chunk in ids.chunks(rows_per_batch(1)) {
        let mut builder =
            QueryBuilder::<Sqlite>::new(format!("DELETE FROM {} WHERE {} IN (", table, column));
        {
            let mut separated = builder.separated(", ");
            for id in chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");
        }
        builder.build().execute(&mut **tx).await?;
    }
    Ok(())
}

/// Insert relationship rows, ignoring rows whose composite primary key exists.
async fn insert_relationships(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    table: &str,
    left_column: &str,
    right_column: &str,
    rows: &[(String, String)],
) -> Result<(), sqlx::Error> {
    let sql_prefix = format!(
        "INSERT OR IGNORE INTO {} ({}, {}) ",
        table, left_column, right_column
    );
    batched_upsert(tx, rows, 2, &sql_prefix, "", |separated, (left, right)| {
        separated.push_bind(left).push_bind(right);
    })
    .await
}

/// Import all JSONL files using a provided SQLite transaction.
///
/// This is SQLite-specific because it uses raw SQL queries within a transaction.
///
/// Every entity type is upserted with batched multi-row statements. Upserts on
/// tables that have an `updated_at` column carry a last-write-wins guard
/// (`excluded.updated_at > <table>.updated_at`) so older data never overwrites
/// newer data and re-importing the same file is a no-op.
async fn import_all_with_transaction(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input_dir: &Path,
) -> Result<ImportSummary, Box<dyn std::error::Error + Send + Sync>> {
    let mut summary = ImportSummary::default();

    // Import order (with deferred FK, this doesn't matter, but keep logical):
    // 1. Projects (no FK dependencies)
    // 2. Repos (can reference projects via project_repo M:N)
    // 3. Task Lists (references projects)
    // 4. Tasks (references task_lists and optionally parent tasks)
    // 5. Notes (can reference projects and repos)
    // 6. Skills (can reference projects via project_skill M:N)

    // ========== Import Projects ==========
    let projects_file = input_dir.join("projects.jsonl");
    if projects_file.exists() {
        let mut reader = JsonlReader::open(&projects_file)?;
        let batch_size = rows_per_batch(8);
        loop {
            let projects: Vec<Project> = read_batch(&mut reader, batch_size)?;
            if projects.is_empty() {
                break;
            }
            let rows: Vec<(&Project, String, String)> = projects
                .iter()
                .map(|project| {
                    Ok((
                        project,
                        serde_json::to_string(&project.tags)?,
                        serde_json::to_string(&project.external_refs)?,
                    ))
                })
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                8,
                "INSERT INTO project (id, title, description, tags, external_refs, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    description = excluded.description,
                    tags = excluded.tags,
                    external_refs = excluded.external_refs,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE project.updated_at IS NULL OR excluded.updated_at > project.updated_at",
                |separated, (project, tags, external_refs)| {
                    separated
                        .push_bind(&project.id)
                        .push_bind(&project.title)
                        .push_bind(&project.description)
                        .push_bind(tags)
                        .push_bind(external_refs)
                        .push_bind(&project.created_at)
                        .push_bind(&project.updated_at)
                        .push_bind(&project.deleted_at);
                },
            )
            .await?;

            summary.projects += projects.len();
        }
    }

    // ========== Import Repos ==========
    let repos_file = input_dir.join("repos.jsonl");
    if repos_file.exists() {
        let mut reader = JsonlReader::open(&repos_file)?;
        let batch_size = rows_per_batch(7);
        loop {
            let repos: Vec<Repo> = read_batch(&mut reader, batch_size)?;
            if repos.is_empty() {
                break;
            }
            let rows: Vec<(&Repo, String)> = repos
                .iter()
                .map(|repo| Ok((repo, serde_json::to_string(&repo.tags)?)))
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                7,
                "INSERT INTO repo (id, remote, path, tags, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    remote = excluded.remote,
                    path = excluded.path,
                    tags = excluded.tags,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE repo.updated_at IS NULL OR excluded.updated_at > repo.updated_at",
                |separated, (repo, tags)| {
                    separated
                        .push_bind(&repo.id)
                        .push_bind(&repo.remote)
                        .push_bind(&repo.path)
                        .push_bind(tags)
                        .push_bind(&repo.created_at)
                        .push_bind(&repo.updated_at)
                        .push_bind(&repo.deleted_at);
                },
            )
            .await?;

            let repo_ids: Vec<String> = repos.iter().map(|repo| repo.id.clone()).collect();
            let links: Vec<(String, String)> = repos
                .iter()
                .flat_map(|repo| {
                    repo.project_ids
                        .iter()
                        .map(move |project_id| (project_id.clone(), repo.id.clone()))
                })
                .collect();
            delete_relationships(tx, "project_repo", "repo_id", &repo_ids).await?;
            insert_relationships(tx, "project_repo", "project_id", "repo_id", &links).await?;

            summary.repos += repos.len();
        }
    }

    // ========== Import Task Lists ==========
    let lists_file = input_dir.join("lists.jsonl");
    if lists_file.exists() {
        let mut reader = JsonlReader::open(&lists_file)?;
        let batch_size = rows_per_batch(12);
        loop {
            let task_lists: Vec<TaskList> = read_batch(&mut reader, batch_size)?;
            if task_lists.is_empty() {
                break;
            }
            let rows: Vec<(&TaskList, String, String)> = task_lists
                .iter()
                .map(|task_list| {
                    Ok((
                        task_list,
                        serde_json::to_string(&task_list.tags)?,
                        serde_json::to_string(&task_list.external_refs)?,
                    ))
                })
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                12,
                "INSERT INTO task_list (id, title, description, notes, project_id, tags, status, external_refs, created_at, updated_at, archived_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    description = excluded.description,
                    notes = excluded.notes,
                    project_id = excluded.project_id,
                    tags = excluded.tags,
                    status = excluded.status,
                    external_refs = excluded.external_refs,
                    updated_at = excluded.updated_at,
                    archived_at = excluded.archived_at,
                    deleted_at = excluded.deleted_at
                  WHERE task_list.updated_at IS NULL OR excluded.updated_at > task_list.updated_at",
                |separated, (task_list, tags, external_refs)| {
                    separated
                        .push_bind(&task_list.id)
                        .push_bind(&task_list.title)
                        .push_bind(&task_list.description)
                        .push_bind(&task_list.notes)
                        .push_bind(&task_list.project_id)
                        .push_bind(tags)
                        .push_bind(task_list.status.to_string())
                        .push_bind(external_refs)
                        .push_bind(&task_list.created_at)
                        .push_bind(&task_list.updated_at)
                        .push_bind(&task_list.archived_at)
                        .push_bind(&task_list.deleted_at);
                },
            )
            .await?;

            let list_ids: Vec<String> = task_lists.iter().map(|list| list.id.clone()).collect();
            let links: Vec<(String, String)> = task_lists
                .iter()
                .flat_map(|list| {
                    list.repo_ids
                        .iter()
                        .map(move |repo_id| (list.id.clone(), repo_id.clone()))
                })
                .collect();
            delete_relationships(tx, "task_list_repo", "task_list_id", &list_ids).await?;
            insert_relationships(tx, "task_list_repo", "task_list_id", "repo_id", &links).await?;

            summary.task_lists += task_lists.len();
        }
    }

    // ========== Import Tasks ==========
    let tasks_file = input_dir.join("tasks.jsonl");
    if tasks_file.exists() {
        let mut reader = JsonlReader::open(&tasks_file)?;
        let batch_size = rows_per_batch(12);
        loop {
            let tasks: Vec<Task> = read_batch(&mut reader, batch_size)?;
            if tasks.is_empty() {
                break;
            }
            let rows: Vec<(&Task, String, String)> = tasks
                .iter()
                .map(|task| {
                    Ok((
                        task,
                        serde_json::to_string(&task.tags)?,
                        serde_json::to_string(&task.external_refs)?,
                    ))
                })
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                12,
                "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    list_id = excluded.list_id,
                    parent_id = excluded.parent_id,
                    title = excluded.title,
                    description = excluded.description,
                    status = excluded.status,
                    priority = excluded.priority,
                    tags = excluded.tags,
                    external_refs = excluded.external_refs,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE task.updated_at IS NULL OR excluded.updated_at > task.updated_at",
                |separated, (task, tags, external_refs)| {
                    separated
                        .push_bind(&task.id)
                        .push_bind(&task.list_id)
                        .push_bind(&task.parent_id)
                        .push_bind(&task.title)
                        .push_bind(&task.description)
                        .push_bind(task.status.to_string())
                        .push_bind(task.priority)
                        .push_bind(tags)
                        .push_bind(external_refs)
                        .push_bind(&task.created_at)
                        .push_bind(&task.updated_at)
                        .push_bind(&task.deleted_at);
                },
            )
            .await?;

            summary.tasks += tasks.len();
        }
    }

    // ========== Import Task Transitions ==========
    let transitions_file = input_dir.join("task_transition_log.jsonl");
    if transitions_file.exists() {
        use context_core::TransitionLog;
        let mut reader = JsonlReader::open(&transitions_file)?;
        let batch_size = rows_per_batch(4);
        loop {
            let transitions: Vec<TransitionLog> = read_batch(&mut reader, batch_size)?;
            if transitions.is_empty() {
                break;
            }

            // `task_transition_log` has no `updated_at` column, so it cannot carry
            // a LWW guard. Re-importing writes identical values.
            batched_upsert(
                tx,
                &transitions,
                4,
                "INSERT INTO task_transition_log (id, task_id, status, transitioned_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    task_id = excluded.task_id,
                    status = excluded.status,
                    transitioned_at = excluded.transitioned_at",
                |separated, transition| {
                    separated
                        .push_bind(&transition.id)
                        .push_bind(&transition.task_id)
                        .push_bind(transition.status.to_string())
                        .push_bind(&transition.transitioned_at);
                },
            )
            .await?;

            summary.transitions += transitions.len();
        }
    }

    // ========== Import Notes ==========
    let notes_file = input_dir.join("notes.jsonl");
    if notes_file.exists() {
        let mut reader = JsonlReader::open(&notes_file)?;
        let batch_size = rows_per_batch(9);
        loop {
            let notes: Vec<Note> = read_batch(&mut reader, batch_size)?;
            if notes.is_empty() {
                break;
            }
            let rows: Vec<(&Note, String)> = notes
                .iter()
                .map(|note| Ok((note, serde_json::to_string(&note.tags)?)))
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                9,
                "INSERT INTO note (id, title, content, tags, parent_id, idx, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    content = excluded.content,
                    tags = excluded.tags,
                    parent_id = excluded.parent_id,
                    idx = excluded.idx,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE note.updated_at IS NULL OR excluded.updated_at > note.updated_at",
                |separated, (note, tags)| {
                    separated
                        .push_bind(&note.id)
                        .push_bind(&note.title)
                        .push_bind(&note.content)
                        .push_bind(tags)
                        .push_bind(&note.parent_id)
                        .push_bind(note.idx)
                        .push_bind(&note.created_at)
                        .push_bind(&note.updated_at)
                        .push_bind(&note.deleted_at);
                },
            )
            .await?;

            let note_ids: Vec<String> = notes.iter().map(|note| note.id.clone()).collect();
            let project_links: Vec<(String, String)> = notes
                .iter()
                .flat_map(|note| {
                    note.project_ids
                        .iter()
                        .map(move |project_id| (project_id.clone(), note.id.clone()))
                })
                .collect();
            let repo_links: Vec<(String, String)> = notes
                .iter()
                .flat_map(|note| {
                    note.repo_ids
                        .iter()
                        .map(move |repo_id| (note.id.clone(), repo_id.clone()))
                })
                .collect();
            delete_relationships(tx, "project_note", "note_id", &note_ids).await?;
            insert_relationships(tx, "project_note", "project_id", "note_id", &project_links)
                .await?;
            delete_relationships(tx, "note_repo", "note_id", &note_ids).await?;
            insert_relationships(tx, "note_repo", "note_id", "repo_id", &repo_links).await?;

            summary.notes += notes.len();
        }

        // Null out orphaned parent_id values before commit. The export may
        // contain notes whose parent was deleted but the parent_id was never
        // nulled. With foreign_keys(true) and defer_foreign_keys, these would
        // fail at commit. One query, no per-note lookups.
        sqlx::query(
            "UPDATE note SET parent_id = NULL WHERE parent_id IS NOT NULL AND parent_id NOT IN (SELECT id FROM note)",
        )
        .execute(&mut **tx)
        .await?;
    }

    // ========== Import Skills ==========
    let skills_file = input_dir.join("skills.jsonl");
    if skills_file.exists() {
        let mut reader = JsonlReader::open(&skills_file)?;
        let batch_size = rows_per_batch(8);
        loop {
            let skills: Vec<Skill> = read_batch(&mut reader, batch_size)?;
            if skills.is_empty() {
                break;
            }
            let rows: Vec<(&Skill, String)> = skills
                .iter()
                .map(|skill| Ok((skill, serde_json::to_string(&skill.tags)?)))
                .collect::<Result<_, serde_json::Error>>()?;

            batched_upsert(
                tx,
                &rows,
                8,
                "INSERT INTO skill (id, name, description, content, tags, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    description = excluded.description,
                    content = excluded.content,
                    tags = excluded.tags,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE skill.updated_at IS NULL OR excluded.updated_at > skill.updated_at",
                |separated, (skill, tags)| {
                    separated
                        .push_bind(&skill.id)
                        .push_bind(&skill.name)
                        .push_bind(&skill.description)
                        .push_bind(&skill.content)
                        .push_bind(tags)
                        .push_bind(&skill.created_at)
                        .push_bind(&skill.updated_at)
                        .push_bind(&skill.deleted_at);
                },
            )
            .await?;

            let skill_ids: Vec<String> = skills.iter().map(|skill| skill.id.clone()).collect();
            let links: Vec<(String, String)> = skills
                .iter()
                .flat_map(|skill| {
                    skill
                        .project_ids
                        .iter()
                        .map(move |project_id| (project_id.clone(), skill.id.clone()))
                })
                .collect();
            delete_relationships(tx, "project_skill", "skill_id", &skill_ids).await?;
            insert_relationships(tx, "project_skill", "project_id", "skill_id", &links).await?;

            summary.skills += skills.len();
        }
    }

    // ========== Import Skill Attachments ==========
    let attachments_file = input_dir.join("skills_attachments.jsonl");
    if attachments_file.exists() {
        use context_core::SkillAttachment;
        let mut reader = JsonlReader::open(&attachments_file)?;
        let batch_size = rows_per_batch(10);
        loop {
            let attachments: Vec<SkillAttachment> = read_batch(&mut reader, batch_size)?;
            if attachments.is_empty() {
                break;
            }

            batched_upsert(
                tx,
                &attachments,
                10,
                "INSERT INTO skill_attachment (id, skill_id, type, filename, content, content_hash, mime_type, created_at, updated_at, deleted_at) ",
                " ON CONFLICT(id) DO UPDATE SET
                    skill_id = excluded.skill_id,
                    type = excluded.type,
                    filename = excluded.filename,
                    content = excluded.content,
                    content_hash = excluded.content_hash,
                    mime_type = excluded.mime_type,
                    updated_at = excluded.updated_at,
                    deleted_at = excluded.deleted_at
                  WHERE skill_attachment.updated_at IS NULL OR excluded.updated_at > skill_attachment.updated_at",
                |separated, attachment| {
                    separated
                        .push_bind(&attachment.id)
                        .push_bind(&attachment.skill_id)
                        .push_bind(&attachment.type_)
                        .push_bind(&attachment.filename)
                        .push_bind(&attachment.content)
                        .push_bind(&attachment.content_hash)
                        .push_bind(&attachment.mime_type)
                        .push_bind(&attachment.created_at)
                        .push_bind(&attachment.updated_at)
                        .push_bind(&attachment.deleted_at);
                },
            )
            .await?;

            summary.attachments += attachments.len();
        }
    }

    Ok(summary)
}

/// Parse a JSON-encoded string array column into a `Vec<String>`.
fn json_string_vec(row: &sqlx::sqlite::SqliteRow, column: &str) -> Vec<String> {
    let raw: Option<String> = row.get(column);
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Current schema version recorded by sqlx migrations.
async fn current_schema_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(pool)
            .await?;
    Ok(version.unwrap_or(0))
}

/// Read the incremental export cutoff from `sync_state`.
///
/// Returns `None` when the table is missing (unmigrated database), when no
/// export has run yet, or when the stored schema version differs from the
/// current one. A schema version change resets the cutoff so the next export
/// is a full export.
async fn read_sync_cutoff(pool: &SqlitePool) -> Result<Option<String>, sqlx::Error> {
    let table_exists: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'sync_state'",
    )
    .fetch_optional(pool)
    .await?;
    if table_exists.is_none() {
        return Ok(None);
    }

    let row: Option<(Option<String>, Option<i64>)> =
        sqlx::query_as("SELECT last_synced_at, schema_version FROM sync_state WHERE id = 1")
            .fetch_optional(pool)
            .await?;

    let Some((last_synced_at, stored_version)) = row else {
        return Ok(None);
    };

    let current_version = current_schema_version(pool).await?;
    if stored_version != Some(current_version) {
        sqlx::query("UPDATE sync_state SET last_synced_at = NULL, schema_version = ? WHERE id = 1")
            .bind(current_version)
            .execute(pool)
            .await?;
        return Ok(None);
    }

    Ok(last_synced_at)
}

/// Advance the incremental export cutoff after a successful export.
async fn write_sync_cutoff(pool: &SqlitePool, timestamp: &str) -> Result<(), sqlx::Error> {
    let current_version = current_schema_version(pool).await?;
    sqlx::query(
        "UPDATE sync_state
         SET last_full_sync_at = CASE WHEN last_synced_at IS NULL THEN ? ELSE last_full_sync_at END,
             last_synced_at = ?,
             schema_version = ?
         WHERE id = 1",
    )
    .bind(timestamp)
    .bind(timestamp)
    .bind(current_version)
    .execute(pool)
    .await?;
    Ok(())
}

/// Stream every row produced by `sql` to `path` as JSONL, one line per row.
///
/// `cutoff` is bound to the single `?` placeholder in `sql`. Pass an empty
/// string for a full export: every non-NULL timestamp sorts after `""`, and
/// the `IS NULL` branch of each predicate keeps rows without a timestamp.
///
/// `map_row` converts a row into the entity that is serialized. Rows are
/// fetched one at a time, so memory stays flat regardless of table size.
async fn export_query<T, F>(
    pool: &SqlitePool,
    path: &Path,
    sql: &'static str,
    cutoff: &str,
    mut map_row: F,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
    F: FnMut(&sqlx::sqlite::SqliteRow) -> Result<T, Box<dyn std::error::Error + Send + Sync>>,
{
    let mut writer = JsonlWriter::create(path)?;
    let mut rows = sqlx::query(sql).bind(cutoff).fetch(pool);
    while let Some(row) = rows.try_next().await? {
        let entity = map_row(&row)?;
        writer.write(&entity)?;
    }
    Ok(writer.finish()?)
}

/// Export database entities to JSONL files using a SQLite pool.
///
/// Export is incremental: when `sync_state.last_synced_at` is set, only rows
/// whose timestamp is greater than that cutoff are written. A NULL cutoff (first
/// sync, or a schema version change) exports every row.
///
/// Export reads every matching row, including soft-deleted tombstones, so that
/// deletions propagate to other machines. Each entity's `deleted_at` value is
/// written to the JSONL output.
async fn export_all_from_pool(
    pool: &SqlitePool,
    output_dir: &Path,
) -> Result<ExportSummary, Box<dyn std::error::Error + Send + Sync>> {
    use context_core::{
        Note, Project, Repo, Skill, SkillAttachment, Task, TaskList, TaskListStatus, TaskStatus,
        TransitionLog,
    };
    use std::str::FromStr;

    // An empty cutoff matches every non-NULL timestamp, so a full export and an
    // incremental export share one SQL shape.
    let cutoff = read_sync_cutoff(pool).await?.unwrap_or_default();

    // Export repos, including tombstones.
    let repos = export_query(
        pool,
        &output_dir.join("repos.jsonl"),
        "SELECT r.id, r.remote, r.path, r.tags, r.created_at, r.updated_at, r.deleted_at,
                COALESCE((SELECT json_group_array(pr.project_id) FROM project_repo pr WHERE pr.repo_id = r.id), '[]') AS project_ids
         FROM repo r
         WHERE r.updated_at IS NULL OR r.updated_at > ?
         ORDER BY r.id",
        &cutoff,
        |row| {
            Ok(Repo {
                id: row.get("id"),
                remote: row.get("remote"),
                path: row.get("path"),
                tags: json_string_vec(row, "tags"),
                project_ids: json_string_vec(row, "project_ids"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: row.get("deleted_at"),
            })
        },
    )
    .await?;

    // Export projects, including tombstones.
    let projects = export_query(
        pool,
        &output_dir.join("projects.jsonl"),
        "SELECT p.id, p.title, p.description, p.tags, p.external_refs, p.created_at, p.updated_at, p.deleted_at,
                COALESCE((SELECT json_group_array(pr.repo_id) FROM project_repo pr WHERE pr.project_id = p.id), '[]') AS repo_ids,
                COALESCE((SELECT json_group_array(tl.id) FROM task_list tl WHERE tl.project_id = p.id), '[]') AS task_list_ids,
                COALESCE((SELECT json_group_array(pn.note_id) FROM project_note pn WHERE pn.project_id = p.id), '[]') AS note_ids
         FROM project p
         WHERE p.updated_at IS NULL OR p.updated_at > ?
         ORDER BY p.id",
        &cutoff,
        |row| {
            Ok(Project {
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                tags: json_string_vec(row, "tags"),
                external_refs: json_string_vec(row, "external_refs"),
                repo_ids: json_string_vec(row, "repo_ids"),
                task_list_ids: json_string_vec(row, "task_list_ids"),
                note_ids: json_string_vec(row, "note_ids"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: row.get("deleted_at"),
            })
        },
    )
    .await?;

    // Export task lists, including tombstones.
    let task_lists = export_query(
        pool,
        &output_dir.join("lists.jsonl"),
        "SELECT tl.id, tl.title, tl.description, tl.notes, tl.tags, tl.external_refs, tl.status, tl.project_id, tl.created_at, tl.updated_at, tl.archived_at, tl.deleted_at,
                COALESCE((SELECT json_group_array(tlr.repo_id) FROM task_list_repo tlr WHERE tlr.task_list_id = tl.id), '[]') AS repo_ids
         FROM task_list tl
         WHERE tl.updated_at IS NULL OR tl.updated_at > ?
         ORDER BY tl.id",
        &cutoff,
        |row| {
            Ok(TaskList {
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                notes: row.get("notes"),
                tags: json_string_vec(row, "tags"),
                external_refs: json_string_vec(row, "external_refs"),
                status: TaskListStatus::from_str(&row.get::<String, _>("status"))
                    .unwrap_or_default(),
                repo_ids: json_string_vec(row, "repo_ids"),
                project_id: row.get("project_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                archived_at: row.get("archived_at"),
                deleted_at: row.get("deleted_at"),
            })
        },
    )
    .await?;

    // Export tasks, including tombstones.
    let tasks = export_query(
        pool,
        &output_dir.join("tasks.jsonl"),
        "SELECT id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, updated_at, deleted_at
         FROM task
         WHERE updated_at IS NULL OR updated_at > ?
         ORDER BY id",
        &cutoff,
        |row| {
            Ok(Task {
                id: row.get("id"),
                list_id: row.get("list_id"),
                parent_id: row.get("parent_id"),
                title: row.get("title"),
                description: row.get("description"),
                status: TaskStatus::from_str(&row.get::<String, _>("status"))
                    .unwrap_or_default(),
                priority: row.get("priority"),
                tags: json_string_vec(row, "tags"),
                external_refs: json_string_vec(row, "external_refs"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: row.get("deleted_at"),
            })
        },
    )
    .await?;

    // Export task transitions. Transitions of tombstoned tasks are excluded so
    // the export cannot reference a task that is not exported as live.
    let transitions = export_query(
        pool,
        &output_dir.join("task_transition_log.jsonl"),
        "SELECT id, task_id, status, transitioned_at
         FROM task_transition_log
         WHERE task_id NOT IN (SELECT id FROM task WHERE deleted_at IS NOT NULL)
           AND (transitioned_at IS NULL OR transitioned_at > ?)
         ORDER BY transitioned_at DESC",
        &cutoff,
        |row| {
            let status_str: String = row.get("status");
            let status =
                TaskStatus::from_str(&status_str).map_err(|e| format!("Invalid status: {}", e))?;
            Ok(TransitionLog {
                id: row.get("id"),
                task_id: row.get("task_id"),
                status,
                transitioned_at: row.get("transitioned_at"),
            })
        },
    )
    .await?;

    // Export notes, including tombstones.
    let notes = export_query(
        pool,
        &output_dir.join("notes.jsonl"),
        "SELECT n.id, n.title, n.content, n.tags, n.parent_id, n.idx, n.created_at, n.updated_at, n.deleted_at,
                COALESCE((SELECT json_group_array(pn.project_id) FROM project_note pn WHERE pn.note_id = n.id), '[]') AS project_ids,
                COALESCE((SELECT json_group_array(nr.repo_id) FROM note_repo nr WHERE nr.note_id = n.id), '[]') AS repo_ids
         FROM note n
         WHERE n.updated_at IS NULL OR n.updated_at > ?
         ORDER BY n.id",
        &cutoff,
        |row| {
            Ok(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                tags: json_string_vec(row, "tags"),
                parent_id: row.get("parent_id"),
                idx: row.get("idx"),
                repo_ids: json_string_vec(row, "repo_ids"),
                project_ids: json_string_vec(row, "project_ids"),
                subnote_count: None,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                deleted_at: row.get("deleted_at"),
            })
        },
    )
    .await?;
    // Export skills and their attachments. Both queries are ordered by skill id
    // and merged in one pass, so only one row of each is held in memory at a
    // time. Attachment filenames are only listed for live attachments;
    // tombstoned attachments are exported separately.
    let mut attachment_rows = sqlx::query(
        "SELECT id, skill_id, type, filename, content, content_hash, mime_type, created_at, updated_at, deleted_at
         FROM skill_attachment
         WHERE updated_at IS NULL OR updated_at > ?
         ORDER BY skill_id, id",
    )
    .bind(&cutoff)
    .fetch(pool);
    let mut skill_rows = sqlx::query(
        "SELECT s.id, s.name, s.description, s.content, s.tags, s.created_at, s.updated_at, s.deleted_at,
                COALESCE((SELECT json_group_array(ps.project_id) FROM project_skill ps WHERE ps.skill_id = s.id), '[]') AS project_ids
         FROM skill s
         WHERE s.updated_at IS NULL OR s.updated_at > ?
         ORDER BY s.id",
    )
    .bind(&cutoff)
    .fetch(pool);

    let mut skill_writer = JsonlWriter::create(&output_dir.join("skills.jsonl"))?;
    let mut attachment_writer = JsonlWriter::create(&output_dir.join("skills_attachments.jsonl"))?;

    let mut next_attachment = attachment_rows.try_next().await?;

    while let Some(skill_row) = skill_rows.try_next().await? {
        let id: String = skill_row.get("id");
        let mut scripts = Vec::new();
        let mut references = Vec::new();
        let mut assets = Vec::new();

        while let Some(attachment_row) = next_attachment.take() {
            let attachment_skill_id: String = attachment_row.get("skill_id");
            if attachment_skill_id > id {
                next_attachment = Some(attachment_row);
                break;
            }

            let attachment = SkillAttachment {
                id: attachment_row.get("id"),
                skill_id: attachment_skill_id,
                type_: attachment_row.get("type"),
                filename: attachment_row.get("filename"),
                content: attachment_row.get("content"),
                content_hash: attachment_row.get("content_hash"),
                mime_type: attachment_row.get("mime_type"),
                created_at: attachment_row.get("created_at"),
                updated_at: attachment_row.get("updated_at"),
                deleted_at: attachment_row.get("deleted_at"),
            };

            if attachment.skill_id == id && attachment.deleted_at.is_none() {
                match attachment.type_.as_str() {
                    "script" => scripts.push(attachment.filename.clone()),
                    "reference" => references.push(attachment.filename.clone()),
                    "asset" => assets.push(attachment.filename.clone()),
                    _ => {}
                }
            }

            attachment_writer.write(&attachment)?;
            next_attachment = attachment_rows.try_next().await?;
        }

        skill_writer.write(&Skill {
            id,
            name: skill_row.get("name"),
            description: skill_row.get("description"),
            content: skill_row.get("content"),
            tags: json_string_vec(&skill_row, "tags"),
            project_ids: json_string_vec(&skill_row, "project_ids"),
            scripts,
            references,
            assets,
            created_at: skill_row.get("created_at"),
            updated_at: skill_row.get("updated_at"),
            deleted_at: skill_row.get("deleted_at"),
        })?;
    }

    // Drain attachments that belong to no exported skill.
    while let Some(attachment_row) = next_attachment.take() {
        let attachment = SkillAttachment {
            id: attachment_row.get("id"),
            skill_id: attachment_row.get("skill_id"),
            type_: attachment_row.get("type"),
            filename: attachment_row.get("filename"),
            content: attachment_row.get("content"),
            content_hash: attachment_row.get("content_hash"),
            mime_type: attachment_row.get("mime_type"),
            created_at: attachment_row.get("created_at"),
            updated_at: attachment_row.get("updated_at"),
            deleted_at: attachment_row.get("deleted_at"),
        };
        attachment_writer.write(&attachment)?;
        next_attachment = attachment_rows.try_next().await?;
    }

    let skills = skill_writer.finish()?;
    let attachments = attachment_writer.finish()?;

    Ok(ExportSummary {
        repos,
        projects,
        task_lists,
        tasks,
        transitions,
        notes,
        skills,
        attachments,
    })
}
