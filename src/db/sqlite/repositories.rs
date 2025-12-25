//! SQLite repository implementations.
//!
//! Each repository holds a reference to the shared connection mutex.

use rusqlite::{Connection, params};
use std::sync::Mutex;

use crate::db::{
    DbError, DbResult, Note, NoteRepository, Project, ProjectRepository, Repo, RepoRepository,
    Task, TaskList, TaskListRepository, TaskRepository,
};

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
        todo!()
    }

    fn get(&self, id: &str) -> DbResult<Repo> {
        todo!()
    }

    fn get_by_remote(&self, remote: &str) -> DbResult<Option<Repo>> {
        todo!()
    }

    fn list(&self) -> DbResult<Vec<Repo>> {
        todo!()
    }

    fn update(&self, repo: &Repo) -> DbResult<()> {
        todo!()
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        todo!()
    }

    fn get_projects(&self, repo_id: &str) -> DbResult<Vec<Project>> {
        todo!()
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
        todo!()
    }

    fn get(&self, id: &str) -> DbResult<TaskList> {
        todo!()
    }

    fn list(&self) -> DbResult<Vec<TaskList>> {
        todo!()
    }

    fn update(&self, task_list: &TaskList) -> DbResult<()> {
        todo!()
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        todo!()
    }

    fn link_project(&self, task_list_id: &str, project_id: &str) -> DbResult<()> {
        todo!()
    }

    fn link_repo(&self, task_list_id: &str, repo_id: &str) -> DbResult<()> {
        todo!()
    }

    fn get_projects(&self, task_list_id: &str) -> DbResult<Vec<Project>> {
        todo!()
    }

    fn get_repos(&self, task_list_id: &str) -> DbResult<Vec<Repo>> {
        todo!()
    }
}

// =============================================================================
// TaskRepository
// =============================================================================

/// SQLite-backed task repository.
pub struct SqliteTaskRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

impl TaskRepository for SqliteTaskRepository<'_> {
    fn create(&self, task: &Task) -> DbResult<()> {
        todo!()
    }

    fn get(&self, id: &str) -> DbResult<Task> {
        todo!()
    }

    fn list_by_list(&self, list_id: &str) -> DbResult<Vec<Task>> {
        todo!()
    }

    fn list_by_parent(&self, parent_id: &str) -> DbResult<Vec<Task>> {
        todo!()
    }

    fn update(&self, task: &Task) -> DbResult<()> {
        todo!()
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        todo!()
    }
}

// =============================================================================
// NoteRepository
// =============================================================================

/// SQLite-backed note repository.
pub struct SqliteNoteRepository<'a> {
    pub(crate) conn: &'a Mutex<Connection>,
}

impl NoteRepository for SqliteNoteRepository<'_> {
    fn create(&self, note: &Note) -> DbResult<()> {
        todo!()
    }

    fn get(&self, id: &str) -> DbResult<Note> {
        todo!()
    }

    fn list(&self) -> DbResult<Vec<Note>> {
        todo!()
    }

    fn update(&self, note: &Note) -> DbResult<()> {
        todo!()
    }

    fn delete(&self, id: &str) -> DbResult<()> {
        todo!()
    }

    fn search(&self, query: &str) -> DbResult<Vec<Note>> {
        todo!()
    }

    fn link_project(&self, note_id: &str, project_id: &str) -> DbResult<()> {
        todo!()
    }

    fn link_repo(&self, note_id: &str, repo_id: &str) -> DbResult<()> {
        todo!()
    }

    fn get_projects(&self, note_id: &str) -> DbResult<Vec<Project>> {
        todo!()
    }

    fn get_repos(&self, note_id: &str) -> DbResult<Vec<Repo>> {
        todo!()
    }
}
