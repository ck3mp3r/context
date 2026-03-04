//! SQLite TransitionLogRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, TaskStatus, TransitionLog};

/// SQLx-backed transition log repository.
pub struct SqliteTransitionLogRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> SqliteTransitionLogRepository<'a> {
    /// Insert a new transition log entry.
    pub async fn insert(&self, log: &TransitionLog) -> DbResult<TransitionLog> {
        // Use provided ID if not empty, otherwise generate one
        let id = if log.id.is_empty() {
            generate_entity_id()
        } else {
            log.id.clone()
        };

        // Use provided timestamp or generate if empty
        let transitioned_at = if log.transitioned_at.is_empty() {
            current_timestamp()
        } else {
            log.transitioned_at.clone()
        };

        let status_str = log.status.to_string();

        sqlx::query(
            "INSERT INTO task_transition_log (id, task_id, status, transitioned_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&log.task_id)
        .bind(&status_str)
        .bind(&transitioned_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(TransitionLog {
            id,
            task_id: log.task_id.clone(),
            status: log.status.clone(),
            transitioned_at,
        })
    }

    /// List all transitions for a task, ordered by transitioned_at DESC (newest first).
    pub async fn list_by_task_id(&self, task_id: &str) -> DbResult<Vec<TransitionLog>> {
        let rows = sqlx::query(
            "SELECT id, task_id, status, transitioned_at
             FROM task_transition_log
             WHERE task_id = ?
             ORDER BY transitioned_at DESC",
        )
        .bind(task_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let mut transitions = Vec::new();
        for row in rows {
            let status_str: String = row.get("status");

            let status = TaskStatus::from_str(&status_str).map_err(|e| DbError::Database {
                message: format!("Invalid status: {}", e),
            })?;

            transitions.push(TransitionLog {
                id: row.get("id"),
                task_id: row.get("task_id"),
                status,
                transitioned_at: row.get("transitioned_at"),
            });
        }

        Ok(transitions)
    }

    /// Delete all transitions for a task.
    /// Note: CASCADE DELETE on the FK should handle this automatically,
    /// but this method is useful for explicit cleanup or testing.
    pub async fn delete_by_task_id(&self, task_id: &str) -> DbResult<()> {
        sqlx::query("DELETE FROM task_transition_log WHERE task_id = ?")
            .bind(task_id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }
}
