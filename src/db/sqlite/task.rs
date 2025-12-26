//! SQLite TaskRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::{DbError, DbResult, ListQuery, ListResult, Task, TaskRepository, TaskStatus};

/// SQLx-backed task repository.
pub struct SqliteTaskRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> TaskRepository for SqliteTaskRepository<'a> {
    async fn create(&self, task: &Task) -> DbResult<()> {
        let status_str = task.status.to_string();

        sqlx::query(
            r#"
            INSERT INTO task (id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task.id)
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.content)
        .bind(status_str)
        .bind(task.priority)
        .bind(&task.created_at)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn get(&self, id: &str) -> DbResult<Task> {
        let row = sqlx::query(
            "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at
             FROM task WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "Task".to_string(),
            id: id.to_string(),
        })?;

        Ok(Task {
            id: row.get("id"),
            list_id: row.get("list_id"),
            parent_id: row.get("parent_id"),
            content: row.get("content"),
            status: {
                let status_str: String = row.get("status");
                TaskStatus::from_str(&status_str).unwrap_or_default()
            },
            priority: row.get("priority"),
            created_at: row.get("created_at"),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
        })
    }

    async fn list_by_list(
        &self,
        list_id: &str,
        query: Option<&ListQuery>,
    ) -> DbResult<ListResult<Task>> {
        let default_query = ListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = [
            "content",
            "status",
            "priority",
            "created_at",
            "completed_at",
        ];

        // Build query with pagination and sorting
        let order_clause = build_order_clause(query, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(query);

        let sql = format!(
            "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at
             FROM task 
             WHERE list_id = ? 
             {} {}",
            order_clause, limit_clause
        );

        // Get paginated results
        let rows = sqlx::query(&sql)
            .bind(list_id)
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Task> = rows
            .into_iter()
            .map(|row| Task {
                id: row.get("id"),
                list_id: row.get("list_id"),
                parent_id: row.get("parent_id"),
                content: row.get("content"),
                status: {
                    let status_str: String = row.get("status");
                    TaskStatus::from_str(&status_str).unwrap_or_default()
                },
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
            })
            .collect();

        // Get total count
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task WHERE list_id = ?")
            .bind(list_id)
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(ListResult {
            items,
            total: total as usize,
            limit: query.limit,
            offset: query.offset.unwrap_or(0),
        })
    }

    async fn list_by_parent(&self, parent_id: &str) -> DbResult<Vec<Task>> {
        let rows = sqlx::query(
            "SELECT id, list_id, parent_id, content, status, priority, created_at, started_at, completed_at
             FROM task 
             WHERE parent_id = ? 
             ORDER BY created_at",
        )
        .bind(parent_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let tasks: Vec<Task> = rows
            .into_iter()
            .map(|row| Task {
                id: row.get("id"),
                list_id: row.get("list_id"),
                parent_id: row.get("parent_id"),
                content: row.get("content"),
                status: {
                    let status_str: String = row.get("status");
                    TaskStatus::from_str(&status_str).unwrap_or_default()
                },
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
            })
            .collect();

        Ok(tasks)
    }

    async fn update(&self, task: &Task) -> DbResult<()> {
        let status_str = task.status.to_string();

        let result = sqlx::query(
            r#"
            UPDATE task 
            SET list_id = ?, parent_id = ?, content = ?, status = ?, priority = ?, 
                started_at = ?, completed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.content)
        .bind(status_str)
        .bind(task.priority)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&task.id)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Task".to_string(),
                id: task.id.clone(),
            });
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM task WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Task".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }
}
