//! SQLite ProjectRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Project, ProjectQuery, ProjectRepository};

/// SQLx-backed project repository.
pub struct SqliteProjectRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> ProjectRepository for SqliteProjectRepository<'a> {
    async fn create(&self, project: &Project) -> DbResult<Project> {
        // Use provided ID if not empty, otherwise generate one
        let id = if project.id.is_empty() {
            generate_entity_id()
        } else {
            project.id.clone()
        };

        // Always generate current timestamps - never use input timestamps
        let created_at = current_timestamp();
        let updated_at = created_at.clone();

        let tags_json = serde_json::to_string(&project.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&project.title)
            .bind(&project.description)
            .bind(&tags_json)
            .bind(&created_at)
            .bind(&updated_at)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(Project {
            id,
            title: project.title.clone(),
            description: project.description.clone(),
            tags: project.tags.clone(),
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at,
            updated_at,
        })
    }

    async fn get(&self, id: &str) -> DbResult<Project> {
        let row = sqlx::query(
            "SELECT id, title, description, tags, created_at, updated_at FROM project WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "Project".to_string(),
            id: id.to_string(),
        })?;

        let tags_json: String = row.get("tags");
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        // Get repo relationships
        let repo_ids: Vec<String> =
            sqlx::query_scalar("SELECT repo_id FROM project_repo WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        // Get task list relationships (now 1:N from task_list.project_id)
        let task_list_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM task_list WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        // Get note relationships
        let note_ids: Vec<String> =
            sqlx::query_scalar("SELECT note_id FROM project_note WHERE project_id = ?")
                .bind(id)
                .fetch_all(self.pool)
                .await
                .map_err(|e| DbError::Database {
                    message: e.to_string(),
                })?;

        Ok(Project {
            id: row.get("id"),
            title: row.get("title"),
            description: row.get("description"),
            tags,
            repo_ids,
            task_list_ids,
            note_ids,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn list(&self, query: Option<&ProjectQuery>) -> DbResult<ListResult<Project>> {
        let default_query = ProjectQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "created_at", "updated_at"];

        let order_clause = build_order_clause(&query.page, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(&query.page);

        // Build conditions and bind values
        let mut conditions: Vec<String> = vec![];
        let mut bind_values: Vec<String> = vec![];

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
                    "SELECT DISTINCT p.id, p.title, p.description, p.tags, p.created_at, p.updated_at \
                     FROM project p, json_each(p.tags) {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT p.id) FROM project p, json_each(p.tags) {}",
                    where_clause
                ),
            )
        } else {
            (
                format!(
                    "SELECT id, title, description, tags, created_at, updated_at FROM project {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM project".to_string(),
            )
        };

        // Execute main query
        let mut sql_query = sqlx::query(&sql);
        for value in &bind_values {
            sql_query = sql_query.bind(value);
        }

        let rows = sql_query
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Project> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Project {
                    id: row.get("id"),
                    title: row.get("title"),
                    description: row.get("description"),
                    tags,
                    repo_ids: vec![],
                    task_list_ids: vec![],
                    note_ids: vec![],
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        // Execute count query
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

    async fn update(&self, project: &Project) -> DbResult<()> {
        let tags_json = serde_json::to_string(&project.tags).unwrap_or_else(|_| "[]".to_string());

        let result = sqlx::query(
            "UPDATE project SET title = ?, description = ?, tags = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&project.title)
        .bind(&project.description)
        .bind(&tags_json)
        .bind(&project.updated_at)
        .bind(&project.id)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Project".to_string(),
                id: project.id.clone(),
            });
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM project WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Project".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }
}
