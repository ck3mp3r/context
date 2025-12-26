//! SQLite ProjectRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::{DbError, DbResult, ListQuery, ListResult, Project, ProjectRepository};

/// SQLx-backed project repository.
pub struct SqliteProjectRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> ProjectRepository for SqliteProjectRepository<'a> {
    async fn create(&self, project: &Project) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO project (id, title, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&project.id)
        .bind(&project.title)
        .bind(&project.description)
        .bind(&project.created_at)
        .bind(&project.updated_at)
        .execute(self.pool)
        .await
        .map_err(|e| DbError::Database {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn get(&self, id: &str) -> DbResult<Project> {
        let row = sqlx::query(
            "SELECT id, title, description, created_at, updated_at FROM project WHERE id = ?",
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

        Ok(Project {
            id: row.get("id"),
            title: row.get("title"),
            description: row.get("description"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn list(&self, query: Option<&ListQuery>) -> DbResult<ListResult<Project>> {
        let default_query = ListQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["title", "created_at", "updated_at"];

        // Build query with pagination and sorting
        let order_clause = build_order_clause(query, &allowed_fields, "created_at");
        let limit_clause = build_limit_offset_clause(query);

        let sql = format!(
            "SELECT id, title, description, created_at, updated_at FROM project {} {}",
            order_clause, limit_clause
        );

        // Get paginated results
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let items: Vec<Project> = rows
            .into_iter()
            .map(|row| Project {
                id: row.get("id"),
                title: row.get("title"),
                description: row.get("description"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        // Get total count
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project")
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

    async fn update(&self, project: &Project) -> DbResult<()> {
        let result = sqlx::query(
            "UPDATE project SET title = ?, description = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&project.title)
        .bind(&project.description)
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
