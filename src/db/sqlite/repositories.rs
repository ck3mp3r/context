//! SQLite repository implementations.
//!
//! Each repository holds a reference to the shared connection mutex.

use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::db::{
    DbError, DbResult, ListQuery, ListResult, Note, NoteRepository, Project, ProjectRepository,
    Repo, RepoRepository, SortOrder, Task, TaskList, TaskListRepository, TaskRepository,
};

// =============================================================================
// Pagination Helpers
// =============================================================================

/// Validate and map a sort field to the actual column name.
/// Returns None for invalid fields (falls back to default).
fn validate_sort_field(field: &str, allowed: &[&str]) -> Option<&'static str> {
    for &allowed_field in allowed {
        if field == allowed_field {
            // Return static str to avoid lifetime issues
            return match field {
                "title" => Some("title"),
                "name" => Some("name"),
                "content" => Some("content"),
                "status" => Some("status"),
                "priority" => Some("priority"),
                "note_type" => Some("note_type"),
                "remote" => Some("remote"),
                "path" => Some("path"),
                "created_at" => Some("created_at"),
                "updated_at" => Some("updated_at"),
                _ => None,
            };
        }
    }
    None
}

/// Build ORDER BY clause from query parameters.
fn build_order_clause(query: &ListQuery, allowed_fields: &[&str], default_field: &str) -> String {
    let sort_field = query
        .sort_by
        .as_deref()
        .and_then(|f| validate_sort_field(f, allowed_fields))
        .unwrap_or(default_field);

    let order = match query.sort_order.unwrap_or(SortOrder::Asc) {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };

    format!("ORDER BY {} {}", sort_field, order)
}

/// Build LIMIT/OFFSET clause from query parameters.
fn build_limit_offset_clause(query: &ListQuery) -> String {
    let mut clause = String::new();
    if let Some(limit) = query.limit {
        clause.push_str(&format!(" LIMIT {}", limit));
    }
    if let Some(offset) = query.offset
        && offset > 0
    {
        clause.push_str(&format!(" OFFSET {}", offset));
    }
    clause
}

/// Helper to execute with connection lock.
fn with_conn<F, T>(conn: &Mutex<Connection>, f: F) -> DbResult<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    let conn = conn.lock().map_err(|e| DbError::Database {
        message: format!("Failed to acquire database lock: {}", e),
    })?;
    f(&conn).map_err(|e| DbError::Database {
        message: e.to_string(),
    })
}

// =============================================================================
// ProjectRepository
// =============================================================================

/// SQLite-backed project repository.
pub struct SqliteProjectRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

impl ProjectRepository for SqliteProjectRepository<'_> {
    fn create(&self, project: &Project) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT INTO project (id, title, description, created_at, updated_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project.id,
                    project.title,
                    project.description,
                    project.created_at,
                    project.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> DbResult<Project> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, title, description, created_at, updated_at 
                 FROM project WHERE id = ?1",
                [id],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        description: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Project".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn list(&self) -> DbResult<Vec<Project>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, description, created_at, updated_at 
                 FROM project ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Project>> {
        let allowed_fields = &["title", "created_at", "updated_at"];
        let order_clause = build_order_clause(query, allowed_fields, "created_at");
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count
            let total: usize = conn.query_row("SELECT COUNT(*) FROM project", [], |row| {
                row.get::<_, i64>(0).map(|v| v as usize)
            })?;

            // Get paginated results
            let sql = format!(
                "SELECT id, title, description, created_at, updated_at FROM project {} {}",
                order_clause, limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn update(&self, project: &Project) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute(
                "UPDATE project SET title = ?1, description = ?2, updated_at = datetime('now') 
                 WHERE id = ?3",
                params![project.title, project.description, project.id],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Project".to_string(),
                id: project.id.clone(),
            },
            other => other,
        })
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute("DELETE FROM project WHERE id = ?1", [id])?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Project".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn link_repo(&self, project_id: &str, repo_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO project_repo (project_id, repo_id) VALUES (?1, ?2)",
                params![project_id, repo_id],
            )?;
            Ok(())
        })
    }

    fn unlink_repo(&self, project_id: &str, repo_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "DELETE FROM project_repo WHERE project_id = ?1 AND repo_id = ?2",
                params![project_id, repo_id],
            )?;
            Ok(())
        })
    }

    fn get_repos(&self, project_id: &str) -> DbResult<Vec<Repo>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT r.id, r.remote, r.path, r.created_at 
                 FROM repo r
                 INNER JOIN project_repo pr ON r.id = pr.repo_id
                 WHERE pr.project_id = ?1
                 ORDER BY r.created_at",
            )?;
            let rows = stmt.query_map([project_id], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    remote: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

// =============================================================================
// RepoRepository
// =============================================================================

/// SQLite-backed repo repository.
pub struct SqliteRepoRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

impl RepoRepository for SqliteRepoRepository<'_> {
    fn create(&self, repo: &Repo) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT INTO repo (id, remote, path, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![repo.id, repo.remote, repo.path, repo.created_at],
            )?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> DbResult<Repo> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, remote, path, created_at FROM repo WHERE id = ?1",
                [id],
                |row| {
                    Ok(Repo {
                        id: row.get(0)?,
                        remote: row.get(1)?,
                        path: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn get_by_remote(&self, remote: &str) -> DbResult<Option<Repo>> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, remote, path, created_at FROM repo WHERE remote = ?1",
                [remote],
                |row| {
                    Ok(Repo {
                        id: row.get(0)?,
                        remote: row.get(1)?,
                        path: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .optional()
        })
    }

    fn list(&self) -> DbResult<Vec<Repo>> {
        with_conn(self.conn, |conn| {
            let mut stmt =
                conn.prepare("SELECT id, remote, path, created_at FROM repo ORDER BY created_at")?;
            let rows = stmt.query_map([], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    remote: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Repo>> {
        let allowed_fields = &["remote", "path", "created_at"];
        let order_clause = build_order_clause(query, allowed_fields, "created_at");
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count
            let total: usize = conn.query_row("SELECT COUNT(*) FROM repo", [], |row| {
                row.get::<_, i64>(0).map(|v| v as usize)
            })?;

            // Get paginated results
            let sql = format!(
                "SELECT id, remote, path, created_at FROM repo {} {}",
                order_clause, limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    remote: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn update(&self, repo: &Repo) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute(
                "UPDATE repo SET remote = ?1, path = ?2 WHERE id = ?3",
                params![repo.remote, repo.path, repo.id],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: repo.id.clone(),
            },
            other => other,
        })
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute("DELETE FROM repo WHERE id = ?1", [id])?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn get_projects(&self, repo_id: &str) -> DbResult<Vec<Project>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.title, p.description, p.created_at, p.updated_at
                 FROM project p
                 INNER JOIN project_repo pr ON p.id = pr.project_id
                 WHERE pr.repo_id = ?1
                 ORDER BY p.created_at",
            )?;
            let rows = stmt.query_map([repo_id], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

// =============================================================================
// TaskListRepository
// =============================================================================

/// SQLite-backed task list repository.
pub struct SqliteTaskListRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

impl TaskListRepository for SqliteTaskListRepository<'_> {
    fn create(&self, task_list: &TaskList) -> DbResult<()> {
        let tags_json = serde_json::to_string(&task_list.tags).unwrap_or_else(|_| "[]".to_string());
        let status_str = match task_list.status {
            crate::db::TaskListStatus::Active => "active",
            crate::db::TaskListStatus::Archived => "archived",
        };

        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT INTO task_list (id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    task_list.id,
                    task_list.name,
                    task_list.description,
                    task_list.notes,
                    tags_json,
                    task_list.external_ref,
                    status_str,
                    task_list.created_at,
                    task_list.updated_at,
                    task_list.archived_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> DbResult<TaskList> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at 
                 FROM task_list WHERE id = ?1",
                [id],
                |row| {
                    let tags_json: String = row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "[]".to_string());
                    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                    let status_str: String = row.get(6)?;
                    let status = match status_str.as_str() {
                        "archived" => crate::db::TaskListStatus::Archived,
                        _ => crate::db::TaskListStatus::Active,
                    };
                    Ok(TaskList {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        notes: row.get(3)?,
                        tags,
                        external_ref: row.get(5)?,
                        status,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        archived_at: row.get(9)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn list(&self) -> DbResult<Vec<TaskList>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at 
                 FROM task_list ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(4)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let status_str: String = row.get(6)?;
                let status = match status_str.as_str() {
                    "archived" => crate::db::TaskListStatus::Archived,
                    _ => crate::db::TaskListStatus::Active,
                };
                Ok(TaskList {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    notes: row.get(3)?,
                    tags,
                    external_ref: row.get(5)?,
                    status,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    archived_at: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<TaskList>> {
        let allowed_fields = &["name", "status", "created_at", "updated_at"];
        let order_clause = build_order_clause(query, allowed_fields, "created_at");
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count
            let total: usize = conn.query_row("SELECT COUNT(*) FROM task_list", [], |row| {
                row.get::<_, i64>(0).map(|v| v as usize)
            })?;

            // Get paginated results
            let sql = format!(
                "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at FROM task_list {} {}",
                order_clause, limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(4)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let status_str: String = row.get(6)?;
                let status = match status_str.as_str() {
                    "archived" => crate::db::TaskListStatus::Archived,
                    _ => crate::db::TaskListStatus::Active,
                };
                Ok(TaskList {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    notes: row.get(3)?,
                    tags,
                    external_ref: row.get(5)?,
                    status,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    archived_at: row.get(9)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn update(&self, task_list: &TaskList) -> DbResult<()> {
        let tags_json = serde_json::to_string(&task_list.tags).unwrap_or_else(|_| "[]".to_string());
        let status_str = match task_list.status {
            crate::db::TaskListStatus::Active => "active",
            crate::db::TaskListStatus::Archived => "archived",
        };

        with_conn(self.conn, |conn| {
            let rows = conn.execute(
                "UPDATE task_list SET name = ?1, description = ?2, notes = ?3, tags = ?4, 
                 external_ref = ?5, status = ?6, archived_at = ?7 
                 WHERE id = ?8",
                params![
                    task_list.name,
                    task_list.description,
                    task_list.notes,
                    tags_json,
                    task_list.external_ref,
                    status_str,
                    task_list.archived_at,
                    task_list.id,
                ],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: task_list.id.clone(),
            },
            other => other,
        })
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute("DELETE FROM task_list WHERE id = ?1", [id])?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn link_project(&self, task_list_id: &str, project_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO project_task_list (project_id, task_list_id) VALUES (?1, ?2)",
                params![project_id, task_list_id],
            )?;
            Ok(())
        })
    }

    fn link_repo(&self, task_list_id: &str, repo_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO task_list_repo (task_list_id, repo_id) VALUES (?1, ?2)",
                params![task_list_id, repo_id],
            )?;
            Ok(())
        })
    }

    fn get_projects(&self, task_list_id: &str) -> DbResult<Vec<Project>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.title, p.description, p.created_at, p.updated_at 
                 FROM project p
                 INNER JOIN project_task_list ptl ON p.id = ptl.project_id
                 WHERE ptl.task_list_id = ?1
                 ORDER BY p.created_at",
            )?;
            let rows = stmt.query_map([task_list_id], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn get_repos(&self, task_list_id: &str) -> DbResult<Vec<Repo>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT r.id, r.remote, r.path, r.created_at 
                 FROM repo r
                 INNER JOIN task_list_repo tlr ON r.id = tlr.repo_id
                 WHERE tlr.task_list_id = ?1
                 ORDER BY r.created_at",
            )?;
            let rows = stmt.query_map([task_list_id], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    remote: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

// =============================================================================
// TaskRepository
// =============================================================================

/// SQLite-backed task repository.
pub struct SqliteTaskRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

/// Convert TaskStatus to database string.
fn task_status_to_str(status: &crate::db::TaskStatus) -> &'static str {
    match status {
        crate::db::TaskStatus::Backlog => "backlog",
        crate::db::TaskStatus::Todo => "todo",
        crate::db::TaskStatus::InProgress => "in_progress",
        crate::db::TaskStatus::Review => "review",
        crate::db::TaskStatus::Done => "done",
        crate::db::TaskStatus::Cancelled => "cancelled",
    }
}

/// Parse TaskStatus from database string.
fn task_status_from_str(s: &str) -> crate::db::TaskStatus {
    match s {
        "todo" => crate::db::TaskStatus::Todo,
        "in_progress" => crate::db::TaskStatus::InProgress,
        "review" => crate::db::TaskStatus::Review,
        "done" => crate::db::TaskStatus::Done,
        "cancelled" => crate::db::TaskStatus::Cancelled,
        _ => crate::db::TaskStatus::Backlog,
    }
}

impl TaskRepository for SqliteTaskRepository<'_> {
    fn create(&self, task: &Task) -> DbResult<()> {
        let status_str = task_status_to_str(&task.status);

        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT INTO task (id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    task.id,
                    task.list_id,
                    task.parent_id,
                    task.content,
                    status_str,
                    task.priority,
                    task.created_at,
                    task.started_at,
                    task.completed_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> DbResult<Task> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at 
                 FROM task WHERE id = ?1",
                [id],
                |row| {
                    let status_str: String = row.get(4)?;
                    Ok(Task {
                        id: row.get(0)?,
                        list_id: row.get(1)?,
                        parent_id: row.get(2)?,
                        content: row.get(3)?,
                        status: task_status_from_str(&status_str),
                        priority: row.get(5)?,
                        created_at: row.get(6)?,
                        started_at: row.get(7)?,
                        completed_at: row.get(8)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Task".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn list_by_list(&self, list_id: &str) -> DbResult<Vec<Task>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at 
                 FROM task WHERE list_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map([list_id], |row| {
                let status_str: String = row.get(4)?;
                Ok(Task {
                    id: row.get(0)?,
                    list_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    content: row.get(3)?,
                    status: task_status_from_str(&status_str),
                    priority: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn list_by_list_paginated(
        &self,
        list_id: &str,
        query: &ListQuery,
    ) -> DbResult<ListResult<Task>> {
        let allowed_fields = &["content", "status", "priority", "created_at"];
        let order_clause = build_order_clause(query, allowed_fields, "created_at");
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count for this list
            let total: usize = conn.query_row(
                "SELECT COUNT(*) FROM task WHERE list_id = ?1",
                [list_id],
                |row| row.get::<_, i64>(0).map(|v| v as usize),
            )?;

            // Get paginated results
            let sql = format!(
                "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at 
                 FROM task WHERE list_id = ?1 {} {}",
                order_clause, limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([list_id], |row| {
                let status_str: String = row.get(4)?;
                Ok(Task {
                    id: row.get(0)?,
                    list_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    content: row.get(3)?,
                    status: task_status_from_str(&status_str),
                    priority: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn list_by_parent(&self, parent_id: &str) -> DbResult<Vec<Task>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at 
                 FROM task WHERE parent_id = ?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map([parent_id], |row| {
                let status_str: String = row.get(4)?;
                Ok(Task {
                    id: row.get(0)?,
                    list_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    content: row.get(3)?,
                    status: task_status_from_str(&status_str),
                    priority: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn update(&self, task: &Task) -> DbResult<()> {
        let status_str = task_status_to_str(&task.status);

        with_conn(self.conn, |conn| {
            let rows = conn.execute(
                "UPDATE task SET content = ?1, status = ?2, priority = ?3, 
                 started_at = ?4, completed_at = ?5 
                 WHERE id = ?6",
                params![
                    task.content,
                    status_str,
                    task.priority,
                    task.started_at,
                    task.completed_at,
                    task.id,
                ],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Task".to_string(),
                id: task.id.clone(),
            },
            other => other,
        })
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute("DELETE FROM task WHERE id = ?1", [id])?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Task".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }
}

// =============================================================================
// NoteRepository
// =============================================================================

/// SQLite-backed note repository.
pub struct SqliteNoteRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

/// Convert NoteType to database string.
fn note_type_to_str(note_type: &crate::db::NoteType) -> &'static str {
    match note_type {
        crate::db::NoteType::Manual => "manual",
        crate::db::NoteType::ArchivedTodo => "archived_todo",
        crate::db::NoteType::Scratchpad => "scratchpad",
    }
}

/// Parse NoteType from database string.
fn note_type_from_str(s: &str) -> crate::db::NoteType {
    match s {
        "archived_todo" => crate::db::NoteType::ArchivedTodo,
        "scratchpad" => crate::db::NoteType::Scratchpad,
        _ => crate::db::NoteType::Manual,
    }
}

impl NoteRepository for SqliteNoteRepository<'_> {
    fn create(&self, note: &Note) -> DbResult<()> {
        let tags_json = serde_json::to_string(&note.tags).unwrap_or_else(|_| "[]".to_string());
        let note_type_str = note_type_to_str(&note.note_type);

        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT INTO note (id, title, content, tags, note_type, created_at, updated_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    note.id,
                    note.title,
                    note.content,
                    tags_json,
                    note_type_str,
                    note.created_at,
                    note.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    fn get(&self, id: &str) -> DbResult<Note> {
        with_conn(self.conn, |conn| {
            conn.query_row(
                "SELECT id, title, content, tags, note_type, created_at, updated_at 
                 FROM note WHERE id = ?1",
                [id],
                |row| {
                    let tags_json: String = row
                        .get::<_, Option<String>>(3)?
                        .unwrap_or_else(|| "[]".to_string());
                    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                    let note_type_str: String = row.get(4)?;
                    Ok(Note {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        tags,
                        note_type: note_type_from_str(&note_type_str),
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Note".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn list(&self) -> DbResult<Vec<Note>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, tags, note_type, created_at, updated_at 
                 FROM note ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let note_type_str: String = row.get(4)?;
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    note_type: note_type_from_str(&note_type_str),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Note>> {
        let allowed_fields = &["title", "note_type", "created_at", "updated_at"];
        let order_clause = build_order_clause(query, allowed_fields, "created_at");
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count
            let total: usize = conn.query_row("SELECT COUNT(*) FROM note", [], |row| {
                row.get::<_, i64>(0).map(|v| v as usize)
            })?;

            // Get paginated results
            let sql = format!(
                "SELECT id, title, content, tags, note_type, created_at, updated_at FROM note {} {}",
                order_clause, limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let note_type_str: String = row.get(4)?;
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    note_type: note_type_from_str(&note_type_str),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn update(&self, note: &Note) -> DbResult<()> {
        let tags_json = serde_json::to_string(&note.tags).unwrap_or_else(|_| "[]".to_string());
        let note_type_str = note_type_to_str(&note.note_type);

        with_conn(self.conn, |conn| {
            let rows = conn.execute(
                "UPDATE note SET title = ?1, content = ?2, tags = ?3, note_type = ?4 
                 WHERE id = ?5",
                params![note.title, note.content, tags_json, note_type_str, note.id,],
            )?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Note".to_string(),
                id: note.id.clone(),
            },
            other => other,
        })
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            let rows = conn.execute("DELETE FROM note WHERE id = ?1", [id])?;
            if rows == 0 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
        .map_err(|e| match e {
            DbError::Database { message } if message.contains("no rows") => DbError::NotFound {
                entity_type: "Note".to_string(),
                id: id.to_string(),
            },
            other => other,
        })
    }

    fn search(&self, query: &str) -> DbResult<Vec<Note>> {
        with_conn(self.conn, |conn| {
            // Use FTS5 to search notes. Join with note table to get full data.
            let mut stmt = conn.prepare(
                "SELECT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                 FROM note n
                 INNER JOIN note_fts fts ON n.rowid = fts.rowid
                 WHERE note_fts MATCH ?1
                 ORDER BY rank",
            )?;
            let rows = stmt.query_map([query], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let note_type_str: String = row.get(4)?;
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    note_type: note_type_from_str(&note_type_str),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn search_paginated(
        &self,
        search_query: &str,
        query: &ListQuery,
    ) -> DbResult<ListResult<Note>> {
        let limit_offset = build_limit_offset_clause(query);

        with_conn(self.conn, |conn| {
            // Get total count of search results
            let total: usize = conn.query_row(
                "SELECT COUNT(*) FROM note n
                 INNER JOIN note_fts fts ON n.rowid = fts.rowid
                 WHERE note_fts MATCH ?1",
                [search_query],
                |row| row.get::<_, i64>(0).map(|v| v as usize),
            )?;

            // Get paginated search results (FTS5 orders by rank by default)
            let sql = format!(
                "SELECT n.id, n.title, n.content, n.tags, n.note_type, n.created_at, n.updated_at 
                 FROM note n
                 INNER JOIN note_fts fts ON n.rowid = fts.rowid
                 WHERE note_fts MATCH ?1
                 ORDER BY rank {}",
                limit_offset
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([search_query], |row| {
                let tags_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let note_type_str: String = row.get(4)?;
                Ok(Note {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    note_type: note_type_from_str(&note_type_str),
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(ListResult {
                items,
                total,
                limit: query.limit,
                offset: query.offset.unwrap_or(0),
            })
        })
    }

    fn link_project(&self, note_id: &str, project_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO project_note (project_id, note_id) VALUES (?1, ?2)",
                params![project_id, note_id],
            )?;
            Ok(())
        })
    }

    fn link_repo(&self, note_id: &str, repo_id: &str) -> DbResult<()> {
        with_conn(self.conn, |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO note_repo (note_id, repo_id) VALUES (?1, ?2)",
                params![note_id, repo_id],
            )?;
            Ok(())
        })
    }

    fn get_projects(&self, note_id: &str) -> DbResult<Vec<Project>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.title, p.description, p.created_at, p.updated_at 
                 FROM project p
                 INNER JOIN project_note pn ON p.id = pn.project_id
                 WHERE pn.note_id = ?1
                 ORDER BY p.created_at",
            )?;
            let rows = stmt.query_map([note_id], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn get_repos(&self, note_id: &str) -> DbResult<Vec<Repo>> {
        with_conn(self.conn, |conn| {
            let mut stmt = conn.prepare(
                "SELECT r.id, r.remote, r.path, r.created_at 
                 FROM repo r
                 INNER JOIN note_repo nr ON r.id = nr.repo_id
                 WHERE nr.note_id = ?1
                 ORDER BY r.created_at",
            )?;
            let rows = stmt.query_map([note_id], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    remote: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}
