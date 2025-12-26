//! SQLite TaskRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Task, TaskQuery, TaskRepository, TaskStatus};

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

        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO task (id, list_id, parent_id, content, status, priority, tags, created_at, started_at, completed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.content)
        .bind(status_str)
        .bind(task.priority)
        .bind(&tags_json)
        .bind(&created_at)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(Task {
            id,
            list_id: task.list_id.clone(),
            parent_id: task.parent_id.clone(),
            content: task.content.clone(),
            status: task.status.clone(),
            priority: task.priority,
            tags: task.tags.clone(),
            created_at,
            started_at: task.started_at.clone(),
            completed_at: task.completed_at.clone(),
        })
    }

    async fn get(&self, id: &str) -> DbResult<Task> {
        let row = sqlx::query(
            "SELECT id, list_id, parent_id, content, status, priority, tags, created_at, started_at, completed_at
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
        let allowed_fields = [
            "content",
            "status",
            "priority",
            "created_at",
            "completed_at",
        ];

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

        if let Some(status) = &query.status {
            conditions.push("status = ?".to_string());
            bind_values.push(status.clone());
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
                "SELECT DISTINCT t.id, t.list_id, t.parent_id, t.content, t.status, t.priority, t.tags, t.created_at, t.started_at, t.completed_at
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
                "SELECT id, list_id, parent_id, content, status, priority, tags, created_at, started_at, completed_at
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
        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());

        let result = sqlx::query(
            r#"
            UPDATE task 
            SET list_id = ?, parent_id = ?, content = ?, status = ?, priority = ?, tags = ?,
                started_at = ?, completed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&task.list_id)
        .bind(&task.parent_id)
        .bind(&task.content)
        .bind(status_str)
        .bind(task.priority)
        .bind(&tags_json)
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

/// Convert a database row to a Task model.
fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Task {
    Task {
        id: row.get("id"),
        list_id: row.get("list_id"),
        parent_id: row.get("parent_id"),
        content: row.get("content"),
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
    }
}
