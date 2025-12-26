//! SQLite TaskListRepository implementation.

use std::str::FromStr;

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause, build_tag_filter};
use crate::db::{
    DbError, DbResult, ListQuery, ListResult, TaskList, TaskListRepository, TaskListStatus,
};

/// SQLx-backed task list repository.
pub struct SqliteTaskListRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> TaskListRepository for SqliteTaskListRepository<'a> {
    async fn create(&self, task_list: &TaskList) -> DbResult<()> {
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

        // Validate project_ids exist
        for project_id in &task_list.project_ids {
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
            "INSERT INTO task_list (id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&task_list.id)
        .bind(&task_list.name)
        .bind(&task_list.description)
        .bind(&task_list.notes)
        .bind(&tags_json)
        .bind(&task_list.external_ref)
        .bind(task_list.status.to_string())
        .bind(&task_list.created_at)
        .bind(&task_list.updated_at)
        .bind(&task_list.archived_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Insert task_list <-> repo relationships
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

        // Insert task_list <-> project relationships
        for project_id in &task_list.project_ids {
            sqlx::query("INSERT INTO project_task_list (project_id, task_list_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&task_list.id)
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

        Ok(())
    }

    async fn get(&self, id: &str) -> DbResult<TaskList> {
        // Get the main task_list record
        let row = sqlx::query(
            "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at
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
        let repo_ids: Vec<String> = sqlx::query_scalar(
            "SELECT repo_id FROM task_list_repo WHERE task_list_id = ? ORDER BY repo_id",
        )
        .bind(id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        // Get project relationships
        let project_ids: Vec<String> = sqlx::query_scalar(
            "SELECT project_id FROM project_task_list WHERE task_list_id = ? ORDER BY project_id",
        )
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
            project_ids,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            archived_at: row.get("archived_at"),
        })
    }

    async fn list(&self, query: Option<&ListQuery>) -> DbResult<ListResult<TaskList>> {
        let default_query = ListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["name", "status", "created_at", "updated_at"];

        // Build query components
        let order_clause = build_order_clause(query, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(query);
        let tag_filter = build_tag_filter(query);

        // Build query with optional tag filtering
        let (sql, count_sql) = if tag_filter.where_clause.is_empty() {
            // No tag filtering
            (
                format!(
                    "SELECT id, name, description, notes, tags, external_ref, status, created_at, updated_at, archived_at 
                     FROM task_list {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM task_list".to_string(),
            )
        } else {
            // With tag filtering using json_each
            (
                format!(
                    "SELECT DISTINCT tl.id, tl.name, tl.description, tl.notes, tl.tags, tl.external_ref, tl.status, tl.created_at, tl.updated_at, tl.archived_at 
                     FROM task_list tl, json_each(tl.tags)
                     WHERE {} {} {}",
                    tag_filter.where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT tl.id) FROM task_list tl, json_each(tl.tags) WHERE {}",
                    tag_filter.where_clause
                ),
            )
        };

        // Get paginated results
        let mut query_builder = sqlx::query(&sql);
        for tag in &tag_filter.bind_values {
            query_builder = query_builder.bind(tag);
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
                // Parse tags JSON
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

                // Parse status
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
                    repo_ids: vec![], // TODO: Load relationships if needed for list view
                    project_ids: vec![], // TODO: Load relationships if needed for list view
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    archived_at: row.get("archived_at"),
                }
            })
            .collect();

        // Get total count
        let mut count_query = sqlx::query_scalar(&count_sql);
        for tag in &tag_filter.bind_values {
            count_query = count_query.bind(tag);
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
            limit: query.limit,
            offset: query.offset.unwrap_or(0),
        })
    }

    async fn update(&self, task_list: &TaskList) -> DbResult<()> {
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
                status = ?, updated_at = ?, archived_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&task_list.name)
        .bind(&task_list.description)
        .bind(&task_list.notes)
        .bind(tags_json)
        .bind(&task_list.external_ref)
        .bind(status_str)
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

        // Replace project relationships (delete all, then insert new ones)
        sqlx::query("DELETE FROM project_task_list WHERE task_list_id = ?")
            .bind(&task_list.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        for project_id in &task_list.project_ids {
            sqlx::query("INSERT INTO project_task_list (project_id, task_list_id) VALUES (?, ?)")
                .bind(project_id)
                .bind(&task_list.id)
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

        // Delete related project_task_list relationships
        sqlx::query("DELETE FROM project_task_list WHERE task_list_id = ?")
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

        // Insert the relationship (ignore if it already exists)
        sqlx::query(
            "INSERT OR IGNORE INTO project_task_list (project_id, task_list_id) VALUES (?, ?)",
        )
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
