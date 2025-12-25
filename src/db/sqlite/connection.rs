//! SQLite database connection and migration management.

use refinery::embed_migrations;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use crate::db::{
    Database, DbError, DbResult, NoteRepository, ProjectRepository, RepoRepository,
    TaskListRepository, TaskRepository,
};

// Embed migrations from data/sql/sqlite/ at compile time
embed_migrations!("data/sql/sqlite");

/// SQLite database implementation.
pub struct SqliteDatabase {
    conn: Mutex<Connection>,
}

impl SqliteDatabase {
    /// Open a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> DbResult<Self> {
        let conn = Connection::open(path).map_err(|e| DbError::Connection {
            message: e.to_string(),
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory database (useful for testing).
    pub fn in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| DbError::Connection {
            message: e.to_string(),
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Execute a function with access to the underlying connection.
    ///
    /// This is useful for testing and advanced operations that need
    /// direct database access.
    pub fn with_connection<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Connection) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock().map_err(|e| DbError::Database {
            message: format!("Failed to acquire database lock: {}", e),
        })?;
        f(&conn).map_err(|e| DbError::Database {
            message: e.to_string(),
        })
    }
}

impl Database for SqliteDatabase {
    fn migrate(&self) -> DbResult<()> {
        let mut conn = self.conn.lock().map_err(|e| DbError::Database {
            message: format!("Failed to acquire database lock: {}", e),
        })?;

        migrations::runner()
            .run(&mut *conn)
            .map_err(|e| DbError::Migration {
                message: e.to_string(),
            })?;

        Ok(())
    }

    fn projects(&self) -> &dyn ProjectRepository {
        todo!("Implement projects repository")
    }

    fn repos(&self) -> &dyn RepoRepository {
        todo!("Implement repos repository")
    }

    fn task_lists(&self) -> &dyn TaskListRepository {
        todo!("Implement task_lists repository")
    }

    fn tasks(&self) -> &dyn TaskRepository {
        todo!("Implement tasks repository")
    }

    fn notes(&self) -> &dyn NoteRepository {
        todo!("Implement notes repository")
    }
}
