//! SQLite TaskListRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{
    DbError, DbResult, ListResult, TaskList, TaskListQuery, TaskListRepository, TaskListStatus,
};

/// SQLx-backed task list repository.
pub struct SqliteTaskListRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> TaskListRepository for SqliteTaskListRepository<'a> {
    async fn create(&self, task_list: &TaskList) -> DbResult<TaskList> {
        // Use provided ID if not empty, otherwise generate one
        let id = if task_list.id.is_empty() {
            generate_entity_id()
        } else {
            task_list.id.clone()
        };

        // Always generate current timestamps - never use input timestamps
        let created_at = current_timestamp();
        let updated_at = created_at.clone();

        // Start a transaction for atomic operations
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Validate repo_ids exist
        for repo_id in &task_list.repo_ids {
            let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repo WHERE id = ?)")
                .bind(repo_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

            if !exists {
                return Err(DbError::Database {
                    message: format!("Repo with id '{}' not found", repo_id),
                });
            }
        }

        // Validate project_id exists (if provided)
        if let Some(project_id) = &task_list.project_id {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project WHERE id = ?)")
                    .bind(project_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| DbError::Database {
                        message: e.to_string(),
                    })?;

            if !exists {
                return Err(DbError::Database {
                    message: format!("Project with id '{}' not found", project_id),
                });
            }
        }

        // Insert the task_list record
        let tags_json = serde_json::to_string(&task_list.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        sqlx::query(
            "INSERT INTO task_list (id, name, description, notes, tags, external_ref, status, project_id, created_at, updated_at, archived_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&task_list.name)
        .bind(&task_list.description)
        .bind(&task_list.notes)
        .bind(&tags_json)
        .bind(&task_list.external_ref)
        .bind(task_list.status.to_string())
        .bind(&task_list.project_id)
        .bind(&created_at)
        .bind(&updated_at)
        .bind(&task_list.archived_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Insert task_list <-> repo relationships
        for repo_id in &task_list.repo_ids {
            sqlx::query("INSERT INTO task_list_repo (task_list_id, repo_id) VALUES (?, ?)")
                .bind(&id)
                .bind(repo_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        // Commit transaction
        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(TaskList {
            id,
            name: task_list.name.clone(),
            description: task_list.description.clone(),
            notes: task_list.notes.clone(),
            tags: task_list.tags.clone(),
            external_ref: task_list.external_ref.clone(),
            status: task_list.status.clone(),
            created_at,
            updated_at,
            archived_at: task_list.archived_at.clone(),
            repo_ids: task_list.repo_ids.clone(),
            project_id: task_list.project_id.clone(),
        })
    }

    async fn get(&self, id: &str) -> DbResult<TaskList> {
        // Get the main task_list record
        let row = sqlx::query(
            "SELECT id, name, description, notes, tags, external_ref, status, project_id, created_at, updated_at, archived_at
             FROM task_list WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "TaskList".to_string(),
            id: id.to_string(),
        })?;

        // Parse tags JSON
        let tags_json: String = row.get("tags");
        let tags: Vec<String> =
            serde_json::from_str(&tags_json).map_err(|e| DbError::Database {
                message: format!("Failed to parse tags JSON: {}", e),
            })?;

        // Parse status
        let status_str: String = row.get("status");
        let status = TaskListStatus::from_str(&status_str).map_err(|_| DbError::Database {
            message: format!("Invalid status: {}", status_str),
        })?;

        // Get repo relationships
        let repo_ids: Vec<String> =
            sqlx::query_scalar("SELECT repo_id FROM task_list_repo WHERE task_list_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        Ok(TaskList {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            notes: row.get("notes"),
            tags,
            external_ref: row.get("external_ref"),
            status,
            repo_ids,
            project_id: row.get("project_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            archived_at: row.get("archived_at"),
        })
    }

    async fn list(&self, query: Option<&TaskListQuery>) -> DbResult<ListResult<TaskList>> {
        let default_query = TaskListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["name", "status", "created_at", "updated_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        // Build filter conditions
        let mut conditions: Vec<String> = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(status) = &query.status {
            conditions.push("tl.status = ?".to_string());
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
            (
                format!(
                    "SELECT DISTINCT tl.id, tl.name, tl.description, tl.notes, tl.tags, tl.external_ref, tl.status, tl.created_at, tl.updated_at, tl.archived_at 
                     FROM task_list tl, json_each(tl.tags)
                     {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT tl.id) FROM task_list tl, json_each(tl.tags) {}",
                    where_clause
                ),
            )
        } else if !conditions.is_empty() {
            (
                format!(
                    "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at 
                     FROM task_list tl {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!("SELECT COUNT(*) FROM task_list tl {}", where_clause),
            )
        } else {
            (
                format!(
                    "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at 
                     FROM task_list {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM task_list".to_string(),
            )
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

        let items: Vec<TaskList> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                let status_str: String = row.get("status");
                let status = TaskListStatus::from_str(&status_str).unwrap_or_default();

                TaskList {
                    id: row.get("id"),
                    name: row.get("name"),
                    description: row.get("description"),
                    notes: row.get("notes"),
                    tags,
                    external_ref: row.get("external_ref"),
                    status,
                    repo_ids: vec![],
                    project_id: None,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    archived_at: row.get("archived_at"),
                }
            })
            .collect();

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

    async fn update(&self, task_list: &TaskList) -> DbResult<()> {
        // Fetch current to detect status transitions
        let current = self.get(&task_list.id).await?;

        let mut task_list = task_list.clone();

        // Auto-manage archived_at timestamp based on status transitions
        if task_list.status != current.status {
            match task_list.status {
                TaskListStatus::Archived => {
                    // Archiving - set archived_at only if not already set (idempotent)
                    if task_list.archived_at.is_none() {
                        task_list.archived_at = Some(current_timestamp());
                    }
                }
                TaskListStatus::Active => {
                    // Unarchiving - clear archived_at
                    if current.status == TaskListStatus::Archived {
                        task_list.archived_at = None;
                    }
                }
            }
        }

        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Update the main task_lists record
        let tags_json = serde_json::to_string(&task_list.tags).map_err(|e| DbError::Database {
            message: format!("Failed to serialize tags: {}", e),
        })?;

        let status_str = task_list.status.to_string();

        sqlx::query(
            r#"
            UPDATE task_list 
            SET name = ?, description = ?, notes = ?, tags = ?, external_ref = ?, 
                status = ?, project_id = ?, updated_at = ?, archived_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&task_list.name)
        .bind(&task_list.description)
        .bind(&task_list.notes)
        .bind(tags_json)
        .bind(&task_list.external_ref)
        .bind(status_str)
        .bind(&task_list.project_id)
        .bind(&task_list.updated_at)
        .bind(&task_list.archived_at)
        .bind(&task_list.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Replace repo relationships (delete all, then insert new ones)
        sqlx::query("DELETE FROM task_list_repo WHERE task_list_id = ?")
            .bind(&task_list.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for repo_id in &task_list.repo_ids {
            sqlx::query("INSERT INTO task_list_repo (task_list_id, repo_id) VALUES (?, ?)")
                .bind(&task_list.id)
                .bind(repo_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;
        }

        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Delete related task_list_repo relationships
        sqlx::query("DELETE FROM task_list_repo WHERE task_list_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        // Delete the task_list record
        let result = sqlx::query("DELETE FROM task_list WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: id.to_string(),
            });
        }

        tx.commit().await.map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn link_project(&self, task_list_id: &str, project_id: &str) -> DbResult<()> {
        // Check if task list exists
        let task_list_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM task_list WHERE id = ?")
                .bind(task_list_id)
                .fetch_one(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        if task_list_count == 0 {
            return Err(DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: task_list_id.to_string(),
            });
        }

        // Check if project exists
        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project WHERE id = ?")
            .bind(project_id)
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if project_count == 0 {
            return Err(DbError::NotFound {
                entity_type: "Project".to_string(),
                id: project_id.to_string(),
            });
        }

        // Update the project_id column directly
        sqlx::query("UPDATE task_list SET project_id = ? WHERE id = ?")
            .bind(project_id)
            .bind(task_list_id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn link_repo(&self, task_list_id: &str, repo_id: &str) -> DbResult<()> {
        // Check if task list exists
        let task_list_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM task_list WHERE id = ?")
                .bind(task_list_id)
                .fetch_one(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        if task_list_count == 0 {
            return Err(DbError::NotFound {
                entity_type: "TaskList".to_string(),
                id: task_list_id.to_string(),
            });
        }

        // Check if repo exists
        let repo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repo WHERE id = ?")
            .bind(repo_id)
            .fetch_one(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if repo_count == 0 {
            return Err(DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: repo_id.to_string(),
            });
        }

        // Insert the relationship (ignore if it already exists)
        sqlx::query("INSERT OR IGNORE INTO task_list_repo (task_list_id, repo_id) VALUES (?, ?)")
            .bind(task_list_id)
            .bind(repo_id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(())
    }
}
