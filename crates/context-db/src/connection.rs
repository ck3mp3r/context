//! SQLite database connection and migration management.

use sqlx::{SqlitePool, migrate::MigrateDatabase};
use std::path::Path;

use super::{
    SqliteNoteRepository, SqliteProjectRepository, SqliteRepoRepository, SqliteSyncRepository,
    SqliteTaskListRepository, SqliteTaskRepository, SqliteTransitionLogRepository,
};
use context_core::{
    Database, DbError, DbResult, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists,
    HasTasks, HasTransitionLogs,
};

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

impl HasProjects for SqliteDatabase {
    type Projects<'a> = SqliteProjectRepository<'a>;
    fn projects(&self) -> Self::Projects<'_> {
        SqliteProjectRepository { pool: &self.pool }
    }
}

impl HasRepos for SqliteDatabase {
    type Repos<'a> = SqliteRepoRepository<'a>;
    fn repos(&self) -> Self::Repos<'_> {
        SqliteRepoRepository { pool: &self.pool }
    }
}

impl HasTaskLists for SqliteDatabase {
    type TaskLists<'a> = SqliteTaskListRepository<'a>;
    fn task_lists(&self) -> Self::TaskLists<'_> {
        SqliteTaskListRepository { pool: &self.pool }
    }
}

impl HasTasks for SqliteDatabase {
    type Tasks<'a> = SqliteTaskRepository<'a>;
    fn tasks(&self) -> Self::Tasks<'_> {
        SqliteTaskRepository { pool: &self.pool }
    }
}

impl HasNotes for SqliteDatabase {
    type Notes<'a> = SqliteNoteRepository<'a>;
    fn notes(&self) -> Self::Notes<'_> {
        SqliteNoteRepository { pool: &self.pool }
    }
}

impl HasSync for SqliteDatabase {
    type Sync<'a> = SqliteSyncRepository<'a>;
    fn sync(&self) -> Self::Sync<'_> {
        SqliteSyncRepository { pool: &self.pool }
    }
}

impl HasSkills for SqliteDatabase {
    type Skills<'a> = super::SqliteSkillRepository<'a>;
    fn skills(&self) -> Self::Skills<'_> {
        super::SqliteSkillRepository { pool: &self.pool }
    }
}

impl HasTransitionLogs for SqliteDatabase {
    type TransitionLogs<'a> = SqliteTransitionLogRepository<'a>;
    fn transition_logs(&self) -> Self::TransitionLogs<'_> {
        SqliteTransitionLogRepository { pool: &self.pool }
    }
}

impl Database for SqliteDatabase {
    fn migrate(&self) -> DbResult<()> {
        // Use tokio::task::block_in_place for sync interface compatibility
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.migrate_async().await })
        })
    }
}
