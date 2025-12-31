//! SQLite TaskRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{
    DbError, DbResult, ListResult, Task, TaskQuery, TaskRepository, TaskStats, TaskStatus,
};

/// SQLx-backed task repository.
pub struct SqliteTaskRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> TaskRepository for SqliteTaskRepository<'a> {
    async fn create(&self, task: &Task) -> DbResult<Task> {
        // Use provided ID if not empty, otherwise generate one
        let id = if task.id.is_empty() {
            generate_entity_id()
        } else {
            task.id.clone()
        };

        // Always generate current timestamp - never use input timestamp
        let created_at = current_timestamp();
        let updated_at = created_at.clone();

        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, started_at, completed_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(status_str)
        .bind(task.priority)
        .bind(&tags_json)
        .bind(&created_at)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> DbResult<Task> {
        let row = sqlx::query(
            "SELECT id, list_id, parent_id, title, description, status, priority, tags, created_at, started_at, completed_at, updated_at
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

        Ok(row_to_task(&row))
    }

    async fn list(&self, query: Option<&TaskQuery>) -> DbResult<ListResult<Task>> {
        let default_query = TaskQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "status", "priority", "created_at", "completed_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        // Build filter conditions
        let mut conditions: Vec<String> = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(list_id) = &query.list_id {
            conditions.push("list_id = ?".to_string());
            bind_values.push(list_id.clone());
        }

        if let Some(parent_id) = &query.parent_id {
            conditions.push("parent_id = ?".to_string());
            bind_values.push(parent_id.clone());
        }

        // Filter by task type: "task" (parent_id IS NULL) or "subtask" (parent_id IS NOT NULL)
        if let Some(task_type) = &query.task_type {
            match task_type.as_str() {
                "task" => conditions.push("parent_id IS NULL".to_string()),
                "subtask" => conditions.push("parent_id IS NOT NULL".to_string()),
                _ => {} // Ignore invalid values
            }
        }

        if let Some(status) = &query.status {
            // Handle multiple statuses (comma-separated OR logic)
            let statuses: Vec<&str> = status.split(',').map(|s| s.trim()).collect();
            if statuses.len() == 1 {
                conditions.push("status = ?".to_string());
                bind_values.push(status.clone());
            } else {
                let placeholders: Vec<&str> = statuses.iter().map(|_| "?").collect();
                conditions.push(format!("status IN ({})", placeholders.join(", ")));
                bind_values.extend(statuses.iter().map(|s| s.to_string()));
            }
        }

        // Tag filtering requires json_each join
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());

        if let Some(tags) = &query.tags
            && !tags.is_empty()
        {
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
            bind_values.extend(tags.clone());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Build SQL based on whether we need json_each
        let (sql, count_sql) = if needs_json_each {
            let sql = format!(
                "SELECT DISTINCT t.id, t.list_id, t.parent_id, t.title, t.description, t.status, t.priority, t.tags, t.created_at, t.started_at, t.completed_at, t.updated_at
                 FROM task t, json_each(t.tags)
                 {} {} {}",
                where_clause, order_clause, limit_clause
            );
            let count_sql = format!(
                "SELECT COUNT(DISTINCT t.id) FROM task t, json_each(t.tags) {}",
                where_clause
            );
            (sql, count_sql)
        } else {
            let sql = format!(
                "SELECT id, list_id, parent_id, title, description, status, priority, tags, created_at, started_at, completed_at, updated_at
                 FROM task
                 {} {} {}",
                where_clause, order_clause, limit_clause
            );
            let count_sql = format!("SELECT COUNT(*) FROM task {}", where_clause);
            (sql, count_sql)
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        let rows = query_builder
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Task> = rows.iter().map(row_to_task).collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }

        let total: i64 = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(ListResult {
            items,
            total: total as usize,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }

    async fn update(&self, task: &Task) -> DbResult<()> {
        // Fetch current task to detect status transitions
        let current = self.get(&task.id).await?;

        let mut task = task.clone();

        // Track if status is changing and what the old status was (for cascade)
        let status_changed = task.status != current.status;
        let old_status = current.status.clone();

        // Auto-manage timestamps based on status transitions
        if status_changed {
            match task.status {
                TaskStatus::InProgress => {
                    // Starting work - set started_at only if not already set (idempotent)
                    if task.started_at.is_none() {
                        task.started_at = Some(current_timestamp());
                    }
                    // Clear completed_at if reverting from done
                    if current.status == TaskStatus::Done {
                        task.completed_at = None;
                    }
                }
                TaskStatus::Done => {
                    // Completing task - set completed_at only if not already set (idempotent)
                    if task.completed_at.is_none() {
                        task.completed_at = Some(current_timestamp());
                    }
                }
                _ => {
                    // Moving to any other status from done - clear completed_at
                    if current.status == TaskStatus::Done {
                        task.completed_at = None;
                    }
                }
            }
        }

        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());
        let updated_at = current_timestamp();

        // Start transaction for atomic parent + cascade updates
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Update parent task
        let result = sqlx::query(
            r#"
            UPDATE task 
            SET list_id = ?, parent_id = ?, title = ?, description = ?, status = ?, priority = ?, tags = ?,
                started_at = ?, completed_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(&status_str)
        .bind(task.priority)
        .bind(&tags_json)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&updated_at)
        .bind(&task.id)
        .execute(&mut *tx)
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

        // NOTE: updated_at cascade to parent is handled by SQL trigger
        // (task_cascade_updated_at_to_parent in migration 20251231132607)

        // CASCADE: If status changed and this is a parent task, update matching subtasks
        if status_changed && current.parent_id.is_none() {
            let old_status_str = old_status.to_string();
            let new_status_str = task.status.to_string();

            // Update all subtasks that match the parent's OLD status
            let cascade_result = sqlx::query(
                r#"
                UPDATE task 
                SET status = ?,
                    started_at = CASE 
                        WHEN ? = 'in_progress' AND started_at IS NULL THEN datetime('now')
                        ELSE started_at 
                    END,
                    completed_at = CASE 
                        WHEN ? IN ('done', 'cancelled') AND completed_at IS NULL THEN datetime('now')
                        WHEN ? NOT IN ('done', 'cancelled') THEN NULL
                        ELSE completed_at 
                    END
                WHERE parent_id = ? 
                  AND status = ?
                "#,
            )
            .bind(&new_status_str)
            .bind(&new_status_str)  // For started_at CASE
            .bind(&new_status_str)  // For completed_at CASE (set if done/cancelled)
            .bind(&new_status_str)  // For completed_at CASE (clear if not done/cancelled)
            .bind(&task.id)
            .bind(&old_status_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: format!("Failed to cascade status to subtasks: {}", e),
            })?;

            let rows_affected = cascade_result.rows_affected();
            if rows_affected > 0 {
                tracing::debug!(
                    "Cascaded status change from '{}' to '{}' for {} subtask(s) of parent '{}'",
                    old_status_str,
                    new_status_str,
                    rows_affected,
                    task.id
                );
            }
        }

        // Commit transaction
        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

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

    async fn get_stats_for_list(&self, list_id: &str) -> DbResult<TaskStats> {
        let rows = sqlx::query(
            r#"
            SELECT 
                status,
                COUNT(*) as count
            FROM task
            WHERE list_id = ?
            GROUP BY status
            "#,
        )
        .bind(list_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let mut backlog = 0;
        let mut todo = 0;
        let mut in_progress = 0;
        let mut review = 0;
        let mut done = 0;
        let mut cancelled = 0;
        let mut total = 0;

        for row in rows {
            let status: String = row.get("status");
            let count: i64 = row.get("count");
            let count = count as usize;

            total += count;

            match status.as_str() {
                "backlog" => backlog = count,
                "todo" => todo = count,
                "in_progress" => in_progress = count,
                "review" => review = count,
                "done" => done = count,
                "cancelled" => cancelled = count,
                _ => {}
            }
        }

        Ok(TaskStats {
            list_id: list_id.to_string(),
            total,
            backlog,
            todo,
            in_progress,
            review,
            done,
            cancelled,
        })
    }
}

/// Convert a database row to a Task model.
fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Task {
    Task {
        id: row.get("id"),
        list_id: row.get("list_id"),
        parent_id: row.get("parent_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: {
            let status_str: String = row.get("status");
            TaskStatus::from_str(&status_str).unwrap_or_default()
        },
        priority: row.get("priority"),
        tags: {
            let tags_json: Option<String> = row.get("tags");
            tags_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        },
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        updated_at: row.get("updated_at"),
    }
}
