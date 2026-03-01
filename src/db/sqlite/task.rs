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

/// Returns the allowed transitions from a given status.
fn allowed_transitions(current: &TaskStatus) -> Vec<TaskStatus> {
    match current {
        TaskStatus::Backlog => vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Todo => vec![
            TaskStatus::Backlog,
            TaskStatus::InProgress,
            TaskStatus::Cancelled,
        ],
        TaskStatus::InProgress => vec![
            TaskStatus::Todo,
            TaskStatus::Review,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Review => vec![
            TaskStatus::InProgress,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ],
        TaskStatus::Done => vec![
            TaskStatus::Backlog,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Review,
        ],
        TaskStatus::Cancelled => vec![
            TaskStatus::Backlog,
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Review,
        ],
    }
}

impl<'a> TaskRepository for SqliteTaskRepository<'a> {
    async fn create(&self, task: &Task) -> DbResult<Task> {
        // Use provided ID if not empty, otherwise generate one
        let id = if task.id.is_empty() {
            generate_entity_id()
        } else {
            task.id.clone()
        };

        // Use provided timestamps or generate if None
        let created_at = task.created_at.clone().unwrap_or_else(current_timestamp);
        let updated_at = task.updated_at.clone().unwrap_or_else(current_timestamp);

        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());

        let external_refs_json =
            serde_json::to_string(&task.external_refs).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, started_at, completed_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(&external_refs_json)
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
            "SELECT id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, started_at, completed_at, updated_at
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
            "title",
            "status",
            "priority",
            "created_at",
            "completed_at",
            "updated_at",
        ];

        // Check if we need last_activity_at computed column
        // - When sorting by updated_at, compute activity for proper ordering
        // - Only relevant for parent tasks (task_type=task or not filtering by parent_id)
        let is_sorting_by_updated = query.page.sort_by.as_deref() == Some("updated_at");
        let is_querying_parents = query.task_type.as_deref() == Some("task")
            || (query.parent_id.is_none() && query.task_type.is_none());
        let needs_activity_column = is_sorting_by_updated && is_querying_parents;

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

        // Build SQL based on whether we need json_each or activity column
        let (sql, count_sql) = if needs_json_each {
            let select_cols = if needs_activity_column {
                "DISTINCT t.id, t.list_id, t.parent_id, t.title, t.description, t.status, t.priority, t.tags, t.external_refs, t.created_at, t.started_at, t.completed_at, t.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM task WHERE parent_id = t.id), t.updated_at) AS last_activity_at"
            } else {
                "DISTINCT t.id, t.list_id, t.parent_id, t.title, t.description, t.status, t.priority, t.tags, t.external_refs, t.created_at, t.started_at, t.completed_at, t.updated_at"
            };

            // Replace updated_at in ORDER BY with last_activity_at if we computed it
            let order_clause_adjusted = if needs_activity_column {
                order_clause.replace("updated_at", "last_activity_at")
            } else {
                order_clause.clone()
            };

            let sql = format!(
                "SELECT {}
                 FROM task t, json_each(t.tags)
                 {} {} {}",
                select_cols, where_clause, order_clause_adjusted, limit_clause
            );
            let count_sql = format!(
                "SELECT COUNT(DISTINCT t.id) FROM task t, json_each(t.tags) {}",
                where_clause
            );
            (sql, count_sql)
        } else {
            let select_cols = if needs_activity_column {
                "task.id, task.list_id, task.parent_id, task.title, task.description, task.status, task.priority, task.tags, task.external_refs, task.created_at, task.started_at, task.completed_at, task.updated_at, \
                 COALESCE((SELECT MAX(updated_at) FROM task AS child WHERE child.parent_id = task.id), task.updated_at) AS last_activity_at"
            } else {
                "id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, started_at, completed_at, updated_at"
            };

            // Replace updated_at in ORDER BY with last_activity_at if we computed it
            let order_clause_adjusted = if needs_activity_column {
                order_clause.replace("updated_at", "last_activity_at")
            } else {
                order_clause.clone()
            };

            let sql = format!(
                "SELECT {}
                 FROM task
                 {} {} {}",
                select_cols, where_clause, order_clause_adjusted, limit_clause
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

    async fn search(
        &self,
        search_term: &str,
        query: Option<&TaskQuery>,
    ) -> DbResult<ListResult<Task>> {
        let default_query = TaskQuery::default();
        let query = query.unwrap_or(&default_query);

        // Sanitize FTS5 query (same pattern as task_list.rs and project.rs)
        let fts_query = {
            // Strip FTS5-dangerous characters but preserve alphanumeric, underscore, quotes,
            // whitespace, and non-ASCII (for international text)
            let cleaned = search_term
                .chars()
                .map(|c| {
                    if c.is_alphanumeric()
                        || c == '_'
                        || c == '"'
                        || c.is_whitespace()
                        || !c.is_ascii()
                    {
                        c
                    } else {
                        ' '
                    }
                })
                .collect::<String>();

            // Handle unbalanced quotes (FTS5 syntax error)
            let quote_count = cleaned.chars().filter(|c| *c == '"').count();
            let cleaned = if quote_count % 2 == 0 {
                cleaned
            } else {
                cleaned.replace('"', "")
            };

            // Handle empty/whitespace-only queries
            if cleaned.trim().is_empty() {
                return Ok(ListResult {
                    items: vec![],
                    total: 0,
                    limit: query.page.limit,
                    offset: query.page.offset.unwrap_or(0),
                });
            }

            // Detect advanced search features
            let has_boolean =
                cleaned.contains(" AND ") || cleaned.contains(" OR ") || cleaned.contains(" NOT ");
            let has_phrase = cleaned.contains('"');

            // Apply query transformation
            if has_boolean || has_phrase {
                cleaned
            } else {
                // Simple mode - add prefix matching
                cleaned
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|term| format!("{}*", term))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        };

        let mut bind_values: Vec<String> = vec![fts_query];
        let mut where_conditions: Vec<String> = vec!["task_fts MATCH ?".to_string()];

        // Add list_id filter if specified (REQUIRED for tasks)
        if let Some(ref list_id) = query.list_id {
            where_conditions.push("t.list_id = ?".to_string());
            bind_values.push(list_id.clone());
        }

        // Add status filter if specified
        if let Some(ref status) = query.status {
            // Handle multiple statuses (comma-separated OR logic)
            let statuses: Vec<&str> = status.split(',').map(|s| s.trim()).collect();
            if statuses.len() == 1 {
                where_conditions.push("t.status = ?".to_string());
                bind_values.push(status.clone());
            } else {
                let placeholders: Vec<&str> = statuses.iter().map(|_| "?").collect();
                where_conditions.push(format!("t.status IN ({})", placeholders.join(", ")));
                bind_values.extend(statuses.iter().map(|s| s.to_string()));
            }
        }

        // Add parent_id filter if specified
        if let Some(ref parent_id) = query.parent_id {
            where_conditions.push("t.parent_id = ?".to_string());
            bind_values.push(parent_id.clone());
        }

        // Filter by task type: "task" (parent_id IS NULL) or "subtask" (parent_id IS NOT NULL)
        if let Some(task_type) = &query.task_type {
            match task_type.as_str() {
                "task" => where_conditions.push("t.parent_id IS NULL".to_string()),
                "subtask" => where_conditions.push("t.parent_id IS NOT NULL".to_string()),
                _ => {} // Ignore invalid values
            }
        }

        // Check if we need JOINs for tag filtering
        let needs_json_each = query.tags.as_ref().is_some_and(|t| !t.is_empty());

        // Add tag filter if specified
        if needs_json_each {
            let tags = query.tags.as_ref().unwrap();
            let placeholders: Vec<&str> = tags.iter().map(|_| "?").collect();
            where_conditions.push(format!("json_each.value IN ({})", placeholders.join(", ")));
            bind_values.extend(tags.clone());
        }

        let where_clause = format!("WHERE {}", where_conditions.join(" AND "));

        // Build ORDER BY clause
        let allowed_fields = [
            "title",
            "status",
            "priority",
            "created_at",
            "completed_at",
            "updated_at",
        ];
        let order_clause = {
            let sort_field = query
                .page
                .sort_by
                .as_deref()
                .filter(|f| allowed_fields.contains(f))
                .unwrap_or("created_at");

            let order = match query.page.sort_order.unwrap_or(crate::db::SortOrder::Asc) {
                crate::db::SortOrder::Asc => "ASC",
                crate::db::SortOrder::Desc => "DESC",
            };

            format!("ORDER BY t.{} {}", sort_field, order)
        };

        // Build FROM clause with necessary JOINs
        let from_clause = if needs_json_each {
            "FROM task t INNER JOIN task_fts ON t.id = task_fts.id, json_each(t.tags)"
        } else {
            "FROM task t INNER JOIN task_fts ON t.id = task_fts.id"
        };

        // Count query
        let count_sql = format!(
            "SELECT COUNT(DISTINCT t.id) {} {}",
            from_clause, where_clause
        );

        let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }
        let total = count_query
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })? as usize;

        // Data query with LIMIT/OFFSET
        let limit_clause = build_limit_offset_clause(&query.page);
        let data_sql = format!(
            "SELECT DISTINCT t.id, t.list_id, t.parent_id, t.title, t.description, t.status, t.priority, t.tags, t.external_refs, t.created_at, t.started_at, t.completed_at, t.updated_at
             {} {} {} {}",
            from_clause, where_clause, order_clause, limit_clause
        );

        let mut data_query = sqlx::query(&data_sql);
        for value in &bind_values {
            data_query = data_query.bind(value);
        }

        let rows = data_query
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Map rows to Task objects
        let items: Vec<Task> = rows.iter().map(row_to_task).collect();

        Ok(ListResult {
            items,
            total,
            limit: query.page.limit,
            offset: query.page.offset.unwrap_or(0),
        })
    }

    async fn count(&self) -> DbResult<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task")
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;
        Ok(count as usize)
    }

    async fn update(&self, task: &Task) -> DbResult<()> {
        // Fetch current task to detect status transitions
        let current = self.get(&task.id).await?;

        let mut task = task.clone();

        // Auto-manage timestamps based on status transitions
        // NOTE: We NEVER clear historical timestamps - they represent audit trail
        let status_changed = task.status != current.status;
        if status_changed {
            match task.status {
                TaskStatus::InProgress => {
                    // Starting work - set started_at only if not already set (idempotent)
                    if task.started_at.is_none() {
                        task.started_at = Some(current_timestamp());
                    }
                    // Keep completed_at as historical record (don't clear)
                }
                TaskStatus::Done | TaskStatus::Cancelled => {
                    // Completing task - set completed_at only if not already set (idempotent)
                    if task.completed_at.is_none() {
                        task.completed_at = Some(current_timestamp());
                    }
                }
                _ => {
                    // Keep all timestamps as historical records (don't clear)
                }
            }
        }

        let status_str = task.status.to_string();
        let tags_json = serde_json::to_string(&task.tags).unwrap_or_else(|_| "[]".to_string());

        // Use provided timestamp or generate if None
        let updated_at = task.updated_at.clone().unwrap_or_else(current_timestamp);

        // Update task (no transaction needed - single operation)
        let external_refs_json =
            serde_json::to_string(&task.external_refs).unwrap_or_else(|_| "[]".to_string());

        let result = sqlx::query(
            r#"
            UPDATE task 
            SET list_id = ?, parent_id = ?, title = ?, description = ?, status = ?, priority = ?, tags = ?, external_refs = ?,
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
        .bind(&external_refs_json)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&updated_at)
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

    async fn transition_tasks(
        &self,
        task_ids: &[String],
        target_status: TaskStatus,
    ) -> DbResult<Vec<Task>> {
        // Validate input
        if task_ids.is_empty() {
            return Err(DbError::Validation {
                message: "task_ids cannot be empty".to_string(),
            });
        }

        // Start transaction for atomic operation
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Build IN clause for SQL query
        let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT id, list_id, parent_id, title, description, status, priority, tags, external_refs, created_at, started_at, completed_at, updated_at FROM task WHERE id IN ({})",
            placeholders
        );

        // Fetch all tasks
        let mut query = sqlx::query(&query_str);
        for id in task_ids {
            query = query.bind(id);
        }

        let rows = query
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Check all tasks were found
        if rows.len() != task_ids.len() {
            let found_ids: Vec<String> =
                rows.iter().map(|row| row.get::<String, _>("id")).collect();
            let missing: Vec<_> = task_ids
                .iter()
                .filter(|id| !found_ids.contains(id))
                .collect();
            return Err(DbError::NotFound {
                entity_type: "Task".to_string(),
                id: missing.first().unwrap().to_string(),
            });
        }

        // Parse tasks and validate they all have the same current status
        let tasks: Vec<Task> = rows.iter().map(row_to_task).collect();
        let first_status = &tasks[0].status;

        for task in &tasks {
            if task.status != *first_status {
                return Err(DbError::Validation {
                    message: format!(
                        "All tasks must have the same current status. Found mixed statuses: {:?} and {:?}",
                        first_status, task.status
                    ),
                });
            }
        }

        // Validate transition is allowed
        let allowed = allowed_transitions(first_status);
        if !allowed.contains(&target_status) {
            return Err(DbError::Validation {
                message: format!(
                    "invalid_transition: Cannot transition from {:?} to {:?}",
                    first_status, target_status
                ),
            });
        }

        // Calculate timestamps based on target status
        // NOTE: We NEVER clear historical timestamps (started_at, completed_at)
        // These represent audit trail - when work first started/completed
        let should_set_started = target_status == TaskStatus::InProgress;
        let should_set_completed =
            matches!(target_status, TaskStatus::Done | TaskStatus::Cancelled);

        let target_status_str = target_status.to_string();
        let updated_at = current_timestamp();

        // Update all tasks with timestamp management
        let update_query = format!(
            r#"
            UPDATE task 
            SET status = ?,
                started_at = CASE 
                    WHEN ? = 1 AND started_at IS NULL THEN datetime('now')
                    ELSE started_at 
                END,
                completed_at = CASE 
                    WHEN ? = 1 AND completed_at IS NULL THEN datetime('now')
                    ELSE completed_at 
                END,
                updated_at = ?
            WHERE id IN ({})
            "#,
            placeholders
        );

        let mut update = sqlx::query(&update_query)
            .bind(&target_status_str)
            .bind(if should_set_started { 1 } else { 0 })
            .bind(if should_set_completed { 1 } else { 0 })
            .bind(&updated_at);

        for id in task_ids {
            update = update.bind(id);
        }

        update
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Fetch updated tasks
        let mut fetch_query = sqlx::query(&query_str);
        for id in task_ids {
            fetch_query = fetch_query.bind(id);
        }

        let updated_rows =
            fetch_query
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        let updated_tasks: Vec<Task> = updated_rows.iter().map(row_to_task).collect();

        // Commit transaction
        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(updated_tasks)
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
        external_refs: {
            let external_refs_json: Option<String> = row.get("external_refs");
            external_refs_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        },
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        updated_at: row.get("updated_at"),
    }
}
