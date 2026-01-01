//! SQLite database connection and migration management.

use sqlx::{SqlitePool, migrate::MigrateDatabase};
use std::path::Path;

use super::{
    SqliteNoteRepository, SqliteProjectRepository, SqliteRepoRepository, SqliteSyncRepository,
    SqliteTaskListRepository, SqliteTaskRepository,
};
use crate::db::{Database, DbError, DbResult};

/// SQLite database implementation using SQLx.
///
/// Provides async access to repositories via associated types, avoiding dynamic dispatch.
pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    /// Open a database at the given path.
    pub async fn open<P: AsRef<Path>>(path: P) -> DbResult<Self> {
        let database_url = format!("sqlite:{}", path.as_ref().display());

        // Create database file if it doesn't exist
        if !sqlx::Sqlite::database_exists(&database_url)
            .await
            .map_err(|e| DbError::Connection {
                message: e.to_string(),
            })?
        {
            sqlx::Sqlite::create_database(&database_url)
                .await
                .map_err(|e| DbError::Connection {
                    message: e.to_string(),
                })?;
        }

        let pool = SqlitePool::connect(&database_url)
            .await
            .map_err(|e| DbError::Connection {
                message: e.to_string(),
            })?;

        Ok(Self { pool })
    }

    /// Create an in-memory database (useful for testing).
    pub async fn in_memory() -> DbResult<Self> {
        let pool =
            SqlitePool::connect("sqlite::memory:")
                .await
                .map_err(|e| DbError::Connection {
                    message: e.to_string(),
                })?;
        Ok(Self { pool })
    }

    /// Get a reference to the connection pool.
    ///
    /// This is useful for testing and advanced operations that need
    /// direct database access.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Run migrations asynchronously.
    ///
    /// This is the async version of migrate() for use when async context is available.
    pub async fn migrate_async(&self) -> DbResult<()> {
        sqlx::migrate!("data/sql/sqlite/migrations")
            .run(&self.pool)
            .await
            .map_err(|e| DbError::Migration {
                message: e.to_string(),
            })?;

        Ok(())
    }
}

impl Database for SqliteDatabase {
    type Projects<'a> = SqliteProjectRepository<'a>;
    type Repos<'a> = SqliteRepoRepository<'a>;
    type TaskLists<'a> = SqliteTaskListRepository<'a>;
    type Tasks<'a> = SqliteTaskRepository<'a>;
    type Notes<'a> = SqliteNoteRepository<'a>;
    type Sync<'a> = SqliteSyncRepository<'a>;

    fn migrate(&self) -> DbResult<()> {
        // Use tokio::task::block_in_place for sync interface compatibility
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.migrate_async().await })
        })
    }

    fn projects(&self) -> Self::Projects<'_> {
        SqliteProjectRepository { pool: &self.pool }
    }

    fn repos(&self) -> Self::Repos<'_> {
        SqliteRepoRepository { pool: &self.pool }
    }

    fn task_lists(&self) -> Self::TaskLists<'_> {
        SqliteTaskListRepository { pool: &self.pool }
    }

    fn tasks(&self) -> Self::Tasks<'_> {
        SqliteTaskRepository { pool: &self.pool }
    }

    fn notes(&self) -> Self::Notes<'_> {
        SqliteNoteRepository { pool: &self.pool }
    }

    fn sync(&self) -> Self::Sync<'_> {
        SqliteSyncRepository { pool: &self.pool }
    }
}
