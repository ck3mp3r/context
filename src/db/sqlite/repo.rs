//! SQLite RepoRepository implementation.

use sqlx::{Row, SqlitePool};

use super::helpers::{build_limit_offset_clause, build_order_clause};
use crate::db::utils::{current_timestamp, generate_entity_id};
use crate::db::{DbError, DbResult, ListResult, Repo, RepoQuery, RepoRepository};

/// SQLx-backed repo repository.
pub struct SqliteRepoRepository<'a> {
    pub(crate) pool: &'a SqlitePool,
}

impl<'a> RepoRepository for SqliteRepoRepository<'a> {
    async fn create(&self, repo: &Repo) -> DbResult<Repo> {
        // Use provided ID if not empty, otherwise generate one
        let id = if repo.id.is_empty() {
            generate_entity_id()
        } else {
            repo.id.clone()
        };

        // Always generate current timestamp - never use input timestamp
        let created_at = current_timestamp();

        let tags_json = serde_json::to_string(&repo.tags).unwrap_or_else(|_| "[]".to_string());

        sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&tags_json)
            .bind(&created_at)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        Ok(Repo {
            id,
            remote: repo.remote.clone(),
            path: repo.path.clone(),
            tags: repo.tags.clone(),
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at,
        })
    }

    async fn get(&self, id: &str) -> DbResult<Repo> {
        let row = sqlx::query("SELECT id, remote, path, tags, created_at FROM repo WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        let row = row.ok_or(DbError::NotFound {
            entity_type: "Repo".to_string(),
            id: id.to_string(),
        })?;

        let tags_json: String = row.get("tags");
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(Repo {
            id: row.get("id"),
            remote: row.get("remote"),
            path: row.get("path"),
            tags,
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: row.get("created_at"),
        })
    }

    async fn list(&self, query: Option<&RepoQuery>) -> DbResult<ListResult<Repo>> {
        let default_query = RepoQuery::default();
        let query = query.unwrap_or(&default_query);
        let allowed_fields = ["remote", "path", "created_at"];

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
                    "SELECT DISTINCT r.id, r.remote, r.path, r.tags, r.created_at \
                     FROM repo r, json_each(r.tags) {} {} {}",
                    where_clause, order_clause, limit_clause
                ),
                format!(
                    "SELECT COUNT(DISTINCT r.id) FROM repo r, json_each(r.tags) {}",
                    where_clause
                ),
            )
        } else {
            (
                format!(
                    "SELECT id, remote, path, tags, created_at FROM repo {} {}",
                    order_clause, limit_clause
                ),
                "SELECT COUNT(*) FROM repo".to_string(),
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

        let items: Vec<Repo> = rows
            .into_iter()
            .map(|row| {
                let tags_json: String = row.get("tags");
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Repo {
                    id: row.get("id"),
                    remote: row.get("remote"),
                    path: row.get("path"),
                    tags,
                    project_ids: vec![], // Empty by default - relationships managed separately
                    created_at: row.get("created_at"),
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

    async fn update(&self, repo: &Repo) -> DbResult<()> {
        let tags_json = serde_json::to_string(&repo.tags).unwrap_or_else(|_| "[]".to_string());

        let result = sqlx::query("UPDATE repo SET remote = ?, path = ?, tags = ? WHERE id = ?")
            .bind(&repo.remote)
            .bind(&repo.path)
            .bind(&tags_json)
            .bind(&repo.id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: repo.id.clone(),
            });
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM repo WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await
            .map_err(|e| DbError::Database {
                message: e.to_string(),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound {
                entity_type: "Repo".to_string(),
                id: id.to_string(),
            });
        }

        Ok(())
    }
}
